#![allow(warnings)]
#![feature(core_intrinsics)]
pub mod afl;
mod context;
mod feedback;
pub mod libfuzzer;
mod mutators;
mod stages;
use crate::{DepthInfo, Node, Visitor};
use clap::Parser;
use context::Context;
use feedback::register::RegisterFeedback;
use libafl::{
    corpus::{CachedOnDiskCorpus, Corpus, OnDiskCorpus},
    events::{ClientDescription, EventConfig, Launcher, SimpleEventManager},
    executors::{ExitKind, ForkserverExecutor, InProcessExecutor, InProcessForkExecutor},
    feedback_or, feedback_or_fast,
    feedbacks::{
        CrashFeedback, MaxMapFeedback, MaxMapOneOrFilledFeedback, MaxMapPow2Feedback, TimeFeedback,
        TimeoutFeedback,
    },
    inputs::{HasTargetBytes, Input, TargetBytesConverter},
    monitors::{MultiMonitor, SimpleMonitor},
    mutators::HavocScheduledMutator,
    observers::{CanTrack, HitcountsMapObserver, StdMapObserver, TimeObserver},
    schedulers::{powersched::PowerSchedule, QueueScheduler, StdWeightedScheduler},
    stages::{IfStage, StdMutationalStage, StdPowerMutationalStage},
    state::{HasCorpus, HasCurrentTestcase, StdState},
    BloomInputFilter, Evaluator, Fuzzer, HasMetadata, StdFuzzer,
};
pub use libafl_bolts::current_nanos;
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
use libafl_targets::extra_counters;
use libafl_targets::{AFLppCmpLogMap, AFLppCmpLogObserver};
use mutators::{
    recurse_mutate::AutarkieRecurseMutator, splice::AutarkieSpliceMutator,
    splice_append::AutarkieSpliceAppendMutator,
};
use regex::Regex;
use stages::{
    cmp::CmpLogStage, generate::GenerateStage, minimization::MinimizationStage,
    recursive_minimization::RecursiveMinimizationStage,
};
use std::io::{stderr, stdout, Write};
use std::os::fd::AsRawFd;
use std::str::FromStr;
use std::{cell::RefCell, io::ErrorKind, path::PathBuf, process::Command, rc::Rc, time::Duration};
use std::{env::args, ffi::c_int};

use stages::generate;

#[cfg(not(feature = "libfuzzer"))]
const SHMEM_ENV_VAR: &str = "__AFL_SHM_ID";
pub fn run_fuzzer<I, TC, F>(bytes_converter: TC, harness: Option<F>)
where
    I: Node + Input,
    TC: TargetBytesConverter<I> + Clone,
    F: Fn(&I) -> ExitKind,
{
    #[cfg(not(feature = "libfuzzer"))]
    let monitor = MultiMonitor::new(|s| println!("{s}"));
    // TODO: -close_fd_mask from libfuzzer
    #[cfg(feature = "libfuzzer")]
    let monitor = MultiMonitor::new(create_monitor_closure());
    let shmem_provider = StdShMemProvider::new().expect("Failed to init shared memory");
    #[cfg(not(feature = "libfuzzer"))]
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
        #[cfg(not(feature = "libfuzzer"))]
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

        // Create the shared memory map for comms with the forkserver
        #[cfg(not(feature = "libfuzzer"))]
        let mut shmem_provider = UnixShMemProvider::new().unwrap();
        #[cfg(not(feature = "libfuzzer"))]
        let mut shmem = shmem_provider.new_shmem(map_size).unwrap();
        #[cfg(not(feature = "libfuzzer"))]
        unsafe {
            shmem.write_to_env(SHMEM_ENV_VAR).unwrap();
        }
        #[cfg(not(feature = "libfuzzer"))]
        let shmem_buf = shmem.as_slice_mut();

        // Create an observation channel to keep track of edges hit.
        #[cfg(not(feature = "libfuzzer"))]
        let edges_observer = unsafe {
            HitcountsMapObserver::new(StdMapObserver::new("edges", shmem_buf)).track_indices()
        };
        #[cfg(feature = "libfuzzer")]
        let edges = unsafe { extra_counters() };
        #[cfg(feature = "libfuzzer")]
        let edges_observer =
            StdMapObserver::from_mut_slice("edges", edges.into_iter().next().unwrap())
                .track_indices();

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

        // Create an observation channel to keep track of the execution time.
        let time_observer = TimeObserver::new("time");
        let minimization_stage = MinimizationStage::new(Rc::clone(&visitor), &map_feedback);
        let recursive_minimization_stage =
            RecursiveMinimizationStage::new(Rc::clone(&visitor), &map_feedback);
        let mut feedback = feedback_or!(
            map_feedback,
            TimeFeedback::new(&time_observer),
            RegisterFeedback::new(Rc::clone(&visitor)),
        );

        let mut objective = feedback_or_fast!(CrashFeedback::new());

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
        if !fuzzer_dir.join("cmps").exists() {
            std::fs::create_dir(fuzzer_dir.join("cmps")).unwrap();
        }

        let mut context = Context::new(fuzzer_dir.clone());

        let scheduler = StdWeightedScheduler::with_schedule(
            &mut state,
            &edges_observer,
            Some(PowerSchedule::explore()),
        );
        let observers = tuple_list!(edges_observer, time_observer);
        let scheduler = scheduler.cycling_scheduler();
        // Create our Fuzzer
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

        // Create our Executor
        #[cfg(not(feature = "libfuzzer"))]
        let mut executor = ForkserverExecutor::builder()
            .program(opt.executable.clone())
            .coverage_map_size(map_size)
            .debug_child(opt.debug_child)
            .is_persistent(true)
            .is_deferred_frksrv(true)
            .timeout(Duration::from_millis(opt.hang_timeout))
            .shmem_provider(&mut shmem_provider)
            .target_bytes_converter(bytes_converter.clone())
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

        if let Some(dict_file) = &opt.dict_file {
            let file = std::fs::read_to_string(dict_file).expect("cannot read dict file");
            for entry in file.split("\n") {
                visitor.borrow_mut().register_string(entry.to_string());
            }
        }

        // Read strings from the target if configured
        #[cfg(not(feature = "libfuzzer"))]
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
            println!("We imported {} inputs from disk.", state.corpus().count());
        }

        let mutator = HavocScheduledMutator::with_max_stack_pow(
            tuple_list!(
                // SPLICE
                AutarkieSpliceMutator::new(Rc::clone(&visitor), opt.max_subslice_size),
                AutarkieSpliceMutator::new(Rc::clone(&visitor), opt.max_subslice_size),
                AutarkieSpliceMutator::new(Rc::clone(&visitor), opt.max_subslice_size),
                // RECURSIVE GENERATE
                AutarkieRecurseMutator::new(Rc::clone(&visitor), opt.max_subslice_size),
                AutarkieRecurseMutator::new(Rc::clone(&visitor), opt.max_subslice_size),
                AutarkieRecurseMutator::new(Rc::clone(&visitor), opt.max_subslice_size),
                // SPLICE APPEND
                AutarkieSpliceAppendMutator::new(Rc::clone(&visitor)),
            ),
            5,
        );
        #[cfg(not(feature = "libfuzzer"))]
        let cmplog = {
            // The CmpLog map shared between the CmpLog observer and CmpLog executor
            let mut cmplog_shmem = shmem_provider.uninit_on_shmem::<AFLppCmpLogMap>().unwrap();

            // Let the Forkserver know the CmpLog shared memory map ID.
            unsafe {
                cmplog_shmem.write_to_env("__AFL_CMPLOG_SHM_ID").unwrap();
            }
            let cmpmap = unsafe { OwnedRefMut::from_shmem(&mut cmplog_shmem) };
            // Create the CmpLog observer.
            let cmplog_observer = AFLppCmpLogObserver::new("cmplog", cmpmap, true);
            let cmplog_ref = cmplog_observer.handle();
            // Create the CmpLog executor.
            // Cmplog has 25% execution overhead so we give it double the timeout
            let cmplog_executor = ForkserverExecutor::builder()
                .program(opt.executable.clone())
                .coverage_map_size(map_size)
                .debug_child(opt.debug_child)
                .is_persistent(true)
                .is_deferred_frksrv(true)
                .timeout(Duration::from_millis(opt.hang_timeout * 2))
                .shmem_provider(&mut shmem_provider)
                .target_bytes_converter(bytes_converter.clone())
                .build(tuple_list!(cmplog_observer))
                .unwrap();

            let cb = |_fuzzer: &mut _,
                      _executor: &mut _,
                      state: &mut StdState<CachedOnDiskCorpus<I>, I, StdRand, OnDiskCorpus<I>>,
                      _event_manager: &mut _|
             -> Result<bool, Error> {
                if !opt.cmplog || core.core_id() != *opt.cores.ids.first().unwrap() {
                    return Ok(false);
                }
                let testcase = state.current_testcase()?;
                if testcase.scheduled_count() > 1 {
                    return Ok(false);
                }
                Ok(true)
            };
            IfStage::new(
                cb,
                tuple_list!(stages::cmp::CmpLogStage::new(
                    Rc::clone(&visitor),
                    cmplog_executor,
                    cmplog_ref
                )),
            )
        };
        let generate_stage = GenerateStage::new(Rc::clone(&visitor));

        #[cfg(not(feature = "libfuzzer"))]
        let mut stages = tuple_list!(
            // we mut minimize before calculating testcase score
            minimization_stage,
            recursive_minimization_stage,
            cmplog,
            StdPowerMutationalStage::new(mutator),
            generate_stage
        );

        #[cfg(feature = "libfuzzer")]
        let mut stages = tuple_list!(
            // we mut minimize before calculating testcase score
            minimization_stage,
            recursive_minimization_stage,
            StdPowerMutationalStage::new(mutator),
            generate_stage
        );
        fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)?;
        Err(Error::shutting_down())
    };

    Launcher::builder()
        .cores(&opt.cores)
        .monitor(monitor)
        .run_client(run_client)
        .broker_port(4444)
        .shmem_provider(shmem_provider)
        .configuration(EventConfig::from_name("default"))
        .build()
        .launch();
}

#[derive(Debug, Parser, Clone)]
#[command(
    name = "autarkie",
    about = "autarkie",
    author = "aarnav <aarnavbos@gmail.com>"
)]
struct Opt {
    /// What we wanna fuzz
    #[cfg(not(feature = "libfuzzer"))]
    executable: PathBuf,
    /// Fuzzer output dir; will also load inputs from there
    #[arg(short = 'o')]
    output_dir: PathBuf,

    /// Timeout in ms
    #[arg(short = 't', default_value_t = 1000)]
    hang_timeout: u64,

    /// seed for rng
    #[arg(short = 's')]
    rng_seed: Option<u64>,

    /// debug the child
    #[arg(short = 'd')]
    debug_child: bool,

    #[cfg(not(feature = "libfuzzer"))]
    /// AFL_DUMP_MAP_SIZE + x where x = map bias
    #[arg(short = 'm')]
    map_bias: usize,

    /// Amount of initial inputs to generate
    #[arg(short = 'i', default_value_t = 100)]
    initial_generated_inputs: usize,

    /// Include a generate input stage (advanced)
    #[arg(short = 'g')]
    generate: bool,

    #[arg(short = 'c', value_parser=Cores::from_cmdline)]
    cores: Cores,

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
        use $crate::{Node, Visitor};
        let mut visitor = Visitor::new(
            $crate::fuzzer::current_nanos(),
            $crate::DepthInfo {
                generate: 105,
                iterate: 500,
            },
        );
        <$t>::__autarkie_register(&mut visitor, None, 0);
        visitor.calculate_recursion();
        let gen_depth = visitor.generate_depth();
        loop {
            /* println!(
                "{:?}",
                <$t>::__autarkie_generate(&mut visitor, &mut gen_depth.clone(), &mut 0)
            );
            println!("--------------------------------"); */
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
