#![allow(warnings)]
#![feature(core_intrinsics)]

pub mod afl;
mod context;
mod feedback;
mod hooks;
pub mod libfuzzer;
mod mutators;
mod stages;

use crate::{DepthInfo, Node, Visitor};
use clap::Parser;
use context::{Context, MutationMetadata};
use feedback::register::RegisterFeedback;
use libafl::mutators::I2SRandReplace;
#[cfg(feature = "libfuzzer")]
use libafl::stages::ShadowTracingStage;
use libafl::{
    corpus::{CachedOnDiskCorpus, Corpus, OnDiskCorpus},
    events::{ClientDescription, EventConfig, Launcher, SimpleEventManager},
    executors::{
        ExitKind, ForkserverExecutor, InProcessExecutor, InProcessForkExecutor, ShadowExecutor,
    },
    feedback_or, feedback_or_fast,
    feedbacks::{
        CrashFeedback, MaxMapFeedback, MaxMapOneOrFilledFeedback, MaxMapPow2Feedback, TimeFeedback,
        TimeoutFeedback,
    },
    inputs::{BytesInput, HasTargetBytes, Input, InputConverter, InputToBytes},
    monitors::{MultiMonitor, SimpleMonitor},
    mutators::{
        havoc_mutations, havoc_mutations_no_crossover, tokens_mutations, HavocScheduledMutator,
    },
    observers::{CanTrack, HitcountsMapObserver, StdMapObserver, TimeObserver},
    schedulers::{powersched::PowerSchedule, QueueScheduler, StdWeightedScheduler},
    stages::{IfStage, StdMutationalStage, StdPowerMutationalStage},
    state::{HasCorpus, HasCurrentTestcase, StdState},
    BloomInputFilter, Evaluator, Fuzzer, HasMetadata, StdFuzzerBuilder,
};
pub use libafl_bolts::current_nanos;
use libafl_bolts::tuples::Merge;
use libafl_bolts::AsSlice;
use libafl_bolts::TargetArgs;
use libafl_bolts::{
    core_affinity::{CoreId, Cores},
    fs::get_unique_std_input_file,
    ownedref::OwnedRefMut,
    rands::{RomuDuoJrRand, StdRand},
    shmem::{ShMem, ShMemProvider, StdShMemProvider, UnixShMemProvider},
    tuples::{tuple_list, Handled},
    AsSliceMut, Error,
};
#[cfg(feature = "libfuzzer")]
use libafl_targets::{extra_counters, CmpLogObserver};
#[cfg(feature = "afl")]
use libafl_targets::{AFLppCmpLogMap, AFLppCmpLogObserver};
use mutators::{
    recurse_mutate::{AutarkieRecurseMutator, RECURSE_STACK},
    splice::{AutarkieSpliceMutator, SPLICE_STACK},
    splice_append::{AutarkieSpliceAppendMutator, SPLICE_APPEND_STACK},
};
use regex::Regex;
use stages::{
    binary_mutator::AutarkieBinaryMutatorStage,
    generate::GenerateStage,
    minimization::MinimizationStage,
    mutating::MutatingStageWrapper,
    mutational::AutarkieMutationalStage,
    novelty_minimization::NoveltyMinimizationStage,
    recursive_minimization::RecursiveMinimizationStage,
    stats::{AutarkieStats, StatsStage},
};

use std::io::{stderr, stdout, Write};
use std::os::fd::AsRawFd;
use std::str::FromStr;
use std::{cell::RefCell, io::ErrorKind, path::PathBuf, process::Command, rc::Rc, time::Duration};
use std::{env::args, ffi::c_int};

use stages::generate;

#[cfg(feature = "afl")]
const SHMEM_ENV_VAR: &str = "__AFL_SHM_ID";

#[cfg(any(feature = "libfuzzer", feature = "afl"))]
pub fn run_fuzzer<I, TC, F>(bytes_converter: TC, harness: Option<F>)
where
    I: Node + Input,
    TC: InputToBytes<I> + Clone,
    F: Fn(&I) -> ExitKind,
{
    use hooks::rare_share::RareShare;

    #[cfg(feature = "afl")]
    let monitor = MultiMonitor::new(|s| println!("{s}"));
    // TODO: -close_fd_mask from libfuzzer
    #[cfg(feature = "libfuzzer")]
    let monitor = MultiMonitor::new(create_monitor_closure());
    let shmem_provider = StdShMemProvider::new().expect("Failed to init shared memory");
    #[cfg(feature = "afl")]
    let opt = Opt::parse();
    #[cfg(feature = "libfuzzer")]
    let opt = {
        let mut opt = args().collect::<Vec<_>>();
        opt.remove(1);
        opt.remove(opt.len() - 1);
        Opt::parse_from(opt)
    };

    let run_client = |mut state: Option<_>,
                      mut mgr: _,
                      core: ClientDescription|
     -> Result<(), libafl_bolts::Error> {
        if !opt.output_dir.exists() {
            std::fs::create_dir(&opt.output_dir).unwrap();
        }
        #[cfg(feature = "afl")]
        let map_size = {
            let map_size = Command::new(opt.executable.clone())
                .env("AFL_DUMP_MAP_SIZE", "1")
                .output()
                .expect("target gave no output");
            let map_size = String::from_utf8(map_size.stdout)
                .expect("target returned illegal mapsize")
                .replace("\n", "");
            map_size.parse::<usize>().expect("illegal mapsize output") + opt.map_bias
        };

        let fuzzer_dir = opt.output_dir.join(format!("{}", core.core_id().0));
        match std::fs::create_dir(&fuzzer_dir) {
            Ok(_) => {}
            Err(e) => {
                if !matches!(e.kind(), ErrorKind::AlreadyExists) {
                    panic!("{:?}", e)
                }
            }
        };
        #[cfg(feature = "libfuzzer")]
        let cmplog_observer = CmpLogObserver::new("cmplog", true);
        // Create the shared memory map for comms with the forkserver
        #[cfg(feature = "afl")]
        let mut shmem_provider = UnixShMemProvider::new().unwrap();
        #[cfg(feature = "afl")]
        let mut shmem = shmem_provider.new_shmem(map_size).unwrap();
        #[cfg(feature = "afl")]
        unsafe {
            shmem.write_to_env(SHMEM_ENV_VAR).unwrap();
        }
        #[cfg(feature = "afl")]
        let shmem_buf = shmem.as_slice_mut();

        // Create an observation channel to keep track of edges hit.
        #[cfg(feature = "afl")]
        let edges_observer = unsafe {
            HitcountsMapObserver::new(StdMapObserver::new("edges", shmem_buf))
                .track_indices()
                .track_novelties()
        };
        #[cfg(feature = "libfuzzer")]
        let edges = unsafe { extra_counters() };
        #[cfg(feature = "libfuzzer")]
        let edges_observer =
            StdMapObserver::from_mut_slice("edges", edges.into_iter().next().unwrap())
                .track_indices()
                .track_novelties();

        let seed = opt.rng_seed.unwrap_or(current_nanos());

        // Initialize Autarkie's visitor
        let mut visitor = Visitor::new(
            seed,
            DepthInfo {
                generate: opt.generate_depth,
                iterate: opt.iterate_depth,
            },
        );
        I::__autarkie_register(&mut visitor, None, 0);
        visitor.calculate_recursion();
        let visitor = Rc::new(RefCell::new(visitor));

        // Create a MapFeedback for coverage guided fuzzin'
        let map_feedback = MaxMapFeedback::new(&edges_observer);

        let time_observer = TimeObserver::new("time");
        let cb = |_fuzzer: &mut _,
                  _executor: &mut _,
                  state: &mut StdState<CachedOnDiskCorpus<I>, I, StdRand, OnDiskCorpus<I>>,
                  _event_manager: &mut _|
         -> Result<bool, Error> { Ok(opt.novelty_minimization) };
        let novelty_minimization_stage = IfStage::new(
            cb,
            tuple_list!(NoveltyMinimizationStage::new(
                Rc::clone(&visitor),
                &map_feedback
            )),
        );
        let cb = |_fuzzer: &mut _,
                  _executor: &mut _,
                  state: &mut StdState<CachedOnDiskCorpus<I>, I, StdRand, OnDiskCorpus<I>>,
                  _event_manager: &mut _|
         -> Result<bool, Error> {
            Ok(state.current_testcase_mut()?.scheduled_count() == 0)
        };

        let minimization_stage = IfStage::new(
            cb,
            tuple_list!(
                MinimizationStage::new(Rc::clone(&visitor), &map_feedback),
                RecursiveMinimizationStage::new(Rc::clone(&visitor), &map_feedback),
                novelty_minimization_stage,
            ),
        );
        let mut feedback = feedback_or!(
            map_feedback,
            TimeFeedback::new(&time_observer),
            RegisterFeedback::new(Rc::clone(&visitor), bytes_converter.clone()),
        );

        let mut objective = feedback_or_fast!(CrashFeedback::new(), TimeoutFeedback::new(),);

        // Initialize our State if necessary
        let mut state = state.unwrap_or(
            StdState::new(
                RomuDuoJrRand::with_seed(seed),
                // TODO: configure testcache size
                CachedOnDiskCorpus::<I>::new(fuzzer_dir.join("queue"), 4096).unwrap(),
                OnDiskCorpus::<I>::new(fuzzer_dir.join("crash")).unwrap(),
                &mut feedback,
                &mut objective,
            )
            .unwrap(),
        );

        if !fuzzer_dir.join("chunks").exists() {
            std::fs::create_dir(fuzzer_dir.join("chunks")).unwrap();
        }
        if !fuzzer_dir.join("rendered").exists() {
            std::fs::create_dir(fuzzer_dir.join("rendered")).unwrap();
        }
        if !fuzzer_dir.join("cmps").exists() {
            std::fs::create_dir(fuzzer_dir.join("cmps")).unwrap();
        }

        let mut context = Context::new(fuzzer_dir.clone(), opt.render);

        let scheduler = StdWeightedScheduler::with_schedule(
            &mut state,
            &edges_observer,
            Some(PowerSchedule::explore()),
        );
        let observers = tuple_list!(edges_observer, time_observer);
        let scheduler = scheduler.cycling_scheduler();
        // Create our Fuzzer
        let mut filter = BloomInputFilter::new(5000, 0.0001);
        let mut fuzzer = StdFuzzerBuilder::new()
            .input_filter(filter)
            .bytes_converter(bytes_converter.clone())
            .build(scheduler, feedback, objective)
            .unwrap();

        // Create our Executor
        #[cfg(feature = "afl")]
        let mut executor = ForkserverExecutor::builder()
            .program(opt.executable.clone())
            .coverage_map_size(map_size)
            .debug_child(opt.debug_child)
            .is_persistent(true)
            .is_deferred_frksrv(true)
            .timeout(Duration::from_millis(opt.hang_timeout))
            .shmem_provider(&mut shmem_provider)
            .build(observers)
            .unwrap();
        #[cfg(feature = "libfuzzer")]
        let mut harness = harness.unwrap();
        #[cfg(feature = "libfuzzer")]
        let mut executor = InProcessExecutor::with_timeout(
            &mut harness,
            observers,
            &mut fuzzer,
            &mut state,
            &mut mgr,
            Duration::from_millis(opt.hang_timeout),
        )?;
        #[cfg(feature = "libfuzzer")]
        let mut executor = ShadowExecutor::new(executor, tuple_list!(cmplog_observer));
        // Setup a tracing stage in which we log comparisons
        #[cfg(feature = "libfuzzer")]
        let tracing = ShadowTracingStage::new();

        if let Some(dict_file) = &opt.dict_file {
            let file = std::fs::read_to_string(dict_file).expect("cannot read dict file");
            for entry in file.split("\n") {
                visitor.borrow_mut().register_string(entry.to_string());
            }
        }

        // Read strings from the target if configured
        #[cfg(feature = "afl")]
        if opt.get_strings {
            let string_regex = Regex::new("^[a-zA-Z0-9_]+$").unwrap();
            let strings = Command::new("strings")
                .arg(opt.executable.clone())
                .output()
                .expect("strings gave no output!");
            let strings = String::from_utf8_lossy(&strings.stdout);
            for string in strings.lines().into_iter() {
                if string_regex.is_match(string) {
                    visitor.borrow_mut().register_string(string.to_string());
                }
            }
        }

        // Reload corpus chunks if they exist
        for chunk_dir in std::fs::read_dir(fuzzer_dir.join("chunks"))? {
            let dir = chunk_dir?.path();
            for chunk in std::fs::read_dir(dir)? {
                let path = chunk?.path();
                context.add_existing_chunk(path);
            }
        }
        state.add_metadata(context);
        state.add_metadata(AutarkieStats::default());

        // Reload corpus
        if state.must_load_initial_inputs() {
            state.load_initial_inputs(
                &mut fuzzer,
                &mut executor,
                &mut mgr,
                &[fuzzer_dir.join("queue").clone()],
            )?;
            for _ in 0..opt.initial_generated_inputs {
                let mut metadata = state.metadata_mut::<Context>().expect("fxeZamEw____");
                metadata.generated_input();
                let mut generated = crate::fuzzer::generate::generate(&mut visitor.borrow_mut());
                while generated.is_none() {
                    generated = crate::fuzzer::generate::generate(&mut visitor.borrow_mut());
                }
                fuzzer
                    .evaluate_input(
                        &mut state,
                        &mut executor,
                        &mut mgr,
                        generated.as_ref().expect("dVoSuGRU____"),
                    )
                    .unwrap();
            }
            let mut metadata = state.metadata_mut::<Context>().expect("fxeZamEw____");
            metadata.default_input();
            println!("We imported {} inputs from disk.", state.corpus().count());
        }

        let splice_mutator = AutarkieSpliceMutator::new(Rc::clone(&visitor), opt.max_subslice_size);
        let recursion_mutator =
            AutarkieRecurseMutator::new(Rc::clone(&visitor), opt.max_subslice_size);
        let append_mutator = AutarkieSpliceAppendMutator::new(Rc::clone(&visitor));
        let cb = |_fuzzer: &mut _,
                  _executor: &mut _,
                  _state: &mut StdState<CachedOnDiskCorpus<I>, I, StdRand, OnDiskCorpus<I>>,
                  _event_manager: &mut _|
         -> Result<bool, Error> { Ok(opt.generate_stage) };
        let generate_stage = IfStage::new(cb, tuple_list!(GenerateStage::new(Rc::clone(&visitor))));
        let afl_stage = AutarkieBinaryMutatorStage::new(
            havoc_mutations_no_crossover(),
            7,
            MutationMetadata::Afl,
        );
        let i2s = AutarkieBinaryMutatorStage::new(
            tuple_list!(I2SRandReplace::new()),
            7,
            MutationMetadata::I2S,
        );
        // TODO: I2S for AFL
        #[cfg(feature = "afl")]
        let mut stages = tuple_list!(
            minimization_stage,
            MutatingStageWrapper::new(i2s, Rc::clone(&visitor)),
            MutatingStageWrapper::new(
                AutarkieMutationalStage::new(append_mutator, SPLICE_APPEND_STACK),
                Rc::clone(&visitor)
            ),
            MutatingStageWrapper::new(
                AutarkieMutationalStage::new(recursion_mutator, RECURSE_STACK),
                Rc::clone(&visitor)
            ),
            MutatingStageWrapper::new(
                AutarkieMutationalStage::new(splice_mutator, SPLICE_STACK),
                Rc::clone(&visitor)
            ),
            MutatingStageWrapper::new(afl_stage, Rc::clone(&visitor)),
            MutatingStageWrapper::new(generate_stage, Rc::clone(&visitor)),
            StatsStage::new(fuzzer_dir),
        );
        #[cfg(feature = "libfuzzer")]
        let mut stages = tuple_list!(
            minimization_stage,
            tracing,
            MutatingStageWrapper::new(i2s, Rc::clone(&visitor)),
            MutatingStageWrapper::new(
                AutarkieMutationalStage::new(
                    tuple_list!(append_mutator, recursion_mutator, splice_mutator),
                    SPLICE_STACK
                ),
                Rc::clone(&visitor)
            ),
            MutatingStageWrapper::new(generate_stage, Rc::clone(&visitor)),
            MutatingStageWrapper::new(afl_stage, Rc::clone(&visitor)),
            StatsStage::new(fuzzer_dir),
        );
        fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)?;
        Err(Error::shutting_down())
    };

    Launcher::builder()
        .cores(&opt.cores)
        .monitor(monitor)
        .run_client(run_client)
        .broker_port(opt.broker_port)
        .shmem_provider(shmem_provider)
        .configuration(EventConfig::from_name("default"))
        .build()
        .launch_with_hooks(tuple_list!(RareShare::new(opt.skip_count)));
}

#[derive(Debug, Parser, Clone)]
#[command(
    name = "autarkie",
    about = "autarkie",
    author = "aarnav <aarnavbos@gmail.com>"
)]
struct Opt {
    /// What we wanna fuzz
    #[cfg(feature = "afl")]
    executable: PathBuf,
    /// Fuzzer output dir; will also load inputs from there
    #[arg(short = 'o')]
    output_dir: PathBuf,

    /// Timeout in milliseconds
    #[arg(short = 't', default_value_t = 1000)]
    hang_timeout: u64,

    /// Share an entry only every n entries
    #[arg(short = 'K', default_value_t = 100)]
    skip_count: usize,

    /// seed for rng
    #[arg(short = 's')]
    rng_seed: Option<u64>,

    /// debug the child
    #[arg(short = 'd')]
    debug_child: bool,

    /// Render for other fuzzers
    #[arg(short = 'r')]
    render: bool,

    /// broker port
    #[arg(short = 'p', default_value_t = 4000)]
    broker_port: u16,

    #[cfg(feature = "afl")]
    /// AFL_DUMP_MAP_SIZE + x where x = map bias
    #[arg(short = 'm')]
    map_bias: usize,

    /// Amount of initial inputs to generate
    #[arg(short = 'i', default_value_t = 100)]
    initial_generated_inputs: usize,

    /// Include a generate input stage (advanced)
    #[arg(short = 'g')]
    generate_stage: bool,

    #[arg(short = 'c', value_parser=Cores::from_cmdline)]
    cores: Cores,

    #[arg(short = 'n')]
    novelty_minimization: bool,

    /// Max iterate depth when generating iterable nodes (advanced)
    #[arg(short = 'I', default_value_t = 5)]
    iterate_depth: usize,

    /// Max subslice length when doing partial iterable splicing (advanced)
    #[arg(short = 'z', default_value_t = 15)]
    max_subslice_size: usize,

    /// Max generate depth when generating recursive nodes (advanced)
    #[arg(short = 'G', default_value_t = 2)]
    generate_depth: usize,

    /// AFL++ LLVM_DICT2FILE
    #[arg(short = 'x')]
    dict_file: Option<PathBuf>,

    /// Use AFL++'s cmplog feature
    #[arg(short = 'e')]
    cmplog: bool,

    /// capture strings from the binary (only useful if you have a lot of String nodes)
    #[arg(short = 'S')]
    get_strings: bool,
}

#[macro_export]
macro_rules! debug_grammar {
    ($t:ty) => {
        fn main() {
            use autarkie::{Node, Visitor};
            let mut visitor = Visitor::new(
                $crate::fuzzer::current_nanos(),
                $crate::DepthInfo {
                    generate: 2,
                    iterate: 5,
                },
            );
            <$t>::__autarkie_register(&mut visitor, None, 0);
            visitor.calculate_recursion();
            let gen_depth = visitor.generate_depth();
            loop {
                println!(
                    "{:?}",
                    <$t>::__autarkie_generate(&mut visitor, &mut gen_depth.clone(), &mut 0)
                );
                println!("--------------------------------");
                std::thread::sleep(Duration::from_millis(500))
            }
        }
    };
}
#[cfg(feature = "libfuzzer")]
fn create_monitor_closure() -> impl Fn(&str) + Clone {
    #[cfg(unix)]
    let stderr_fd = std::os::fd::RawFd::from_str(&std::env::var(STDERR_FD_VAR).unwrap()).unwrap(); // set in main
    move |s| {
        #[cfg(unix)]
        {
            use std::os::fd::FromRawFd;

            // unfortunate requirement to meet Clone... thankfully, this does not
            // generate effectively any overhead (no allocations, calls get merged)
            let mut stderr = unsafe { std::fs::File::from_raw_fd(stderr_fd) };
            writeln!(stderr, "{s}").expect("Could not write to stderr???");
            std::mem::forget(stderr); // do not close the descriptor!
        }
        #[cfg(not(unix))]
        eprintln!("{s}");
    }
}
#[cfg(feature = "libfuzzer")]
/// Communicate the stderr duplicated fd to subprocesses
pub const STDERR_FD_VAR: &str = "_LIBAFL_LIBFUZZER_STDERR_FD";
