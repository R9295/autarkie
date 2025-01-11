#![allow(warnings)]
#![feature(core_intrinsics)]
mod context;
mod feedback;
mod mutators;
mod stages;
use clap::Parser;
use context::Context;
use feedback::register::RegisterFeedback;
use libafl::{
    corpus::{CachedOnDiskCorpus, Corpus, OnDiskCorpus},
    events::{ClientDescription, EventConfig, Launcher, SimpleEventManager},
    executors::ForkserverExecutor,
    feedback_or, feedback_or_fast,
    feedbacks::{
        CrashFeedback, MaxMapOneOrFilledFeedback, MaxMapPow2Feedback, TimeFeedback, TimeoutFeedback,
    },
    inputs::{Input, TargetBytesConverter},
    monitors::{MultiMonitor, SimpleMonitor},
    mutators::StdScheduledMutator,
    observers::{CanTrack, HitcountsMapObserver, StdMapObserver, TimeObserver},
    schedulers::{powersched::PowerSchedule, StdWeightedScheduler},
    stages::{IfStage, StdPowerMutationalStage},
    state::{HasCorpus, HasCurrentTestcase, StdState},
    Evaluator, Fuzzer, HasMetadata, StdFuzzer,
};
use libafl_bolts::{
    core_affinity::{CoreId, Cores},
    current_nanos,
    fs::get_unique_std_input_file,
    ownedref::OwnedRefMut,
    rands::{RomuDuoJrRand, StdRand},
    shmem::{ShMem, ShMemProvider, StdShMemProvider, UnixShMemProvider},
    tuples::{tuple_list, Handled},
    AsSliceMut, Error,
};
use libafl_targets::{AFLppCmpLogMap, AFLppCmpLogObserver};
use mutators::{
    recurse_mutate::ThesisRecurseMutator, splice::ThesisSpliceMutator,
    splice_append::ThesisSpliceAppendMutator,
};

use regex::Regex;
use stages::{
    cmp::CmpLogStage, generate::GenerateStage, minimization::MinimizationStage,
    recursive_minimization::RecursiveMinimizationStage,
};
use std::{cell::RefCell, io::ErrorKind, path::PathBuf, process::Command, rc::Rc, time::Duration};
use thesis::{DepthInfo, Node, Visitor};

use crate::stages::generate::generate;

const SHMEM_ENV_VAR: &str = "__AFL_SHM_ID";
pub fn fuzz<I, TC>(bytes_converter: TC)
where
    I: Node + Input,
    TC: TargetBytesConverter<Input = I> + Clone,
{
    let monitor = MultiMonitor::new(|s| println!("{s}"));
    let shmem_provider = StdShMemProvider::new().expect("Failed to init shared memory");
    /*     let monitor = MultiMonitor::new(|s| {}); */
    let opt = Opt::parse();
    let run_client = |mut state: Option<_>,
                      mut mgr: _,
                      core: ClientDescription|
     -> Result<(), libafl_bolts::Error> {
        if !opt.output_dir.exists() {
            std::fs::create_dir(&opt.output_dir).unwrap();
        }
        let map_size = Command::new(opt.executable.clone())
            .env("AFL_DUMP_MAP_SIZE", "1")
            .output()
            .expect("target gave no output");
        let map_size = String::from_utf8(map_size.stdout)
            .expect("target returned illegal mapsize")
            .replace("\n", "");
        let map_size = map_size.parse::<usize>().expect("illegal mapsize output") + opt.map_bias;
        let fuzzer_dir = opt.output_dir.join(format!("{}", core.core_id().0));
        match std::fs::create_dir(&fuzzer_dir) {
            Ok(_) => {}
            Err(e) => {
                if matches!(e.kind(), ErrorKind::AlreadyExists) {
                } else {
                    panic!("{:?}", e)
                }
            }
        };
        // Create the shared memory map for comms with the forkserver
        let mut shmem_provider = UnixShMemProvider::new().unwrap();
        let mut shmem = shmem_provider.new_shmem(map_size).unwrap();
        shmem.write_to_env(SHMEM_ENV_VAR).unwrap();
        let shmem_buf = shmem.as_slice_mut();

        // Create an observation channel to keep track of edges hit.
        let edges_observer = unsafe {
            HitcountsMapObserver::new(StdMapObserver::new("edges", shmem_buf)).track_indices()
        };
        let seed = opt.rng_seed.unwrap_or(current_nanos());

        let mut visitor = Visitor::new(
            seed,
            DepthInfo {
                expand: 1500,
                generate: 2,
                iterate: 5,
            },
        );
        let visitor = Rc::new(RefCell::new(visitor));
        // Create a MapFeedback for coverage guided fuzzin'
        // We only care if an edge was hit, not how many times
        let map_feedback = MaxMapPow2Feedback::new(&edges_observer);

        // Create an observation channel to keep track of the execution time.
        let time_observer = TimeObserver::new("time");
        let minimization_stage = MinimizationStage::new(Rc::clone(&visitor), &map_feedback);
        let mut feedback = feedback_or!(
            map_feedback,
            TimeFeedback::new(&time_observer),
            RegisterFeedback::new()
        );

        let mut objective = feedback_or_fast!(CrashFeedback::new());

        // Initialize our State if necessary
        let mut state = state.unwrap_or(
            StdState::new(
                RomuDuoJrRand::with_seed(seed),
                // TODO: configure testcache size
                CachedOnDiskCorpus::<I>::new(fuzzer_dir.join("queue"), 2).unwrap(),
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
        let context = Context::new(fuzzer_dir.clone());
        state.add_metadata(context);

        let scheduler = StdWeightedScheduler::with_schedule(
            &mut state,
            &edges_observer,
            Some(PowerSchedule::explore()),
        );
        let scheduler = scheduler.cycling_scheduler();
        let mut executor = ForkserverExecutor::builder()
            .program(opt.executable.clone())
            .coverage_map_size(map_size)
            .debug_child(opt.debug_child)
            .is_persistent(true)
            .is_deferred_frksrv(true)
            .timeout(Duration::from_millis(opt.hang_timeout))
            .shmem_provider(&mut shmem_provider)
            .target_bytes_converter(bytes_converter.clone())
            .build(tuple_list!(edges_observer, time_observer))
            .unwrap();

        // Create our Fuzzer
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);
        if let Some(dict_file) = &opt.dict_file {
            let file = std::fs::read_to_string(dict_file).expect("cannot read dict file");
            for entry in file.split("\n") {
                visitor.borrow_mut().register_string(entry.to_string());
            }
        }
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
        if state.must_load_initial_inputs() {
            state.load_initial_inputs_multicore(
                &mut fuzzer,
                &mut executor,
                &mut mgr,
                &[fuzzer_dir.join("queue").clone()],
                &core.core_id(),
                &opt.cores,
            )?;
            for _ in 0..opt.initial_generated_inputs {
                let generated: I = generate(&mut visitor.borrow_mut());
                fuzzer
                    .evaluate_input(&mut state, &mut executor, &mut mgr, generated)
                    .unwrap();
            }
            println!("We imported {} inputs from disk.", state.corpus().count());
        }

        let mutator = StdScheduledMutator::with_max_stack_pow(
            tuple_list!(
                // SPLICE
                ThesisSpliceMutator::new(Rc::clone(&visitor)),
                ThesisSpliceMutator::new(Rc::clone(&visitor)),
                ThesisSpliceMutator::new(Rc::clone(&visitor)),
                ThesisSpliceMutator::new(Rc::clone(&visitor)),
                ThesisSpliceMutator::new(Rc::clone(&visitor)),
                ThesisSpliceMutator::new(Rc::clone(&visitor)),
                // RECURSIVE GENERATE
                ThesisRecurseMutator::new(Rc::clone(&visitor)),
                ThesisRecurseMutator::new(Rc::clone(&visitor)),
                ThesisRecurseMutator::new(Rc::clone(&visitor)),
                ThesisRecurseMutator::new(Rc::clone(&visitor)),
                ThesisRecurseMutator::new(Rc::clone(&visitor)),
                ThesisRecurseMutator::new(Rc::clone(&visitor)),
                // SPLICE APPEND
                ThesisSpliceAppendMutator::new(Rc::clone(&visitor)),
            ),
            3,
        );
        // The CmpLog map shared between the CmpLog observer and CmpLog executor
        let mut cmplog_shmem = shmem_provider.uninit_on_shmem::<AFLppCmpLogMap>().unwrap();

        // Let the Forkserver know the CmpLog shared memory map ID.
        cmplog_shmem.write_to_env("__AFL_CMPLOG_SHM_ID").unwrap();
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
                  state: &mut StdState<I, CachedOnDiskCorpus<I>, StdRand, OnDiskCorpus<I>>,
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
        let cmplog = IfStage::new(
            cb,
            tuple_list!(stages::cmp::CmpLogStage::new(
                Rc::clone(&visitor),
                cmplog_executor,
                cmplog_ref
            )),
        );

        let mut stages = tuple_list!(
            // we mut minimize before calculating testcase score
            minimization_stage,
            cmplog,
            StdPowerMutationalStage::new(mutator),
        );

        fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)?;
        Err(Error::shutting_down())
    };
    Launcher::builder()
        .cores(&opt.cores)
        .monitor(monitor)
        .run_client(run_client)
        .shmem_provider(shmem_provider)
        .configuration(EventConfig::from_name("default"))
        .build()
        .launch();
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser, Clone)]
#[command(
    name = "thesis",
    about = "thesis",
    author = "aarnav <aarnavbos@gmail.com>"
)]
struct Opt {
    executable: PathBuf,
    #[arg(short = 'o')]
    output_dir: PathBuf,
    /// Timeout in ms
    #[arg(short = 't', default_value_t = 1000)]
    hang_timeout: u64,

    /// seed for rng
    #[arg(short = 's')]
    rng_seed: Option<u64>,

    #[arg(short = 'd')]
    debug_child: bool,

    #[arg(short = 'm')]
    map_bias: usize,

    #[arg(short = 'g', default_value_t = 100)]
    initial_generated_inputs: usize,

    #[arg(short = 'c', value_parser=Cores::from_cmdline)]
    cores: Cores,

    #[arg(short = 'I', default_value_t = 5)]
    iterate_depth: usize,
    #[arg(short = 'G', default_value_t = 2)]
    generate_depth: usize,

    #[arg(short = 'x')]
    dict_file: Option<PathBuf>,

    #[arg(short = 'e')]
    cmplog: bool,

    #[arg(short = 'S')]
    get_strings: bool,
}

#[macro_export]
macro_rules! debug_grammar {
    ($t:ty) => {
        use thesis::Visitor;
        let mut v = Visitor::new(
            libafl_bolts::current_nanos(),
            thesis::DepthInfo {
                expand: 1500,
                generate: 5,
                iterate: 3,
            },
        );
        let gen_depth = v.generate_depth();
        for _ in 0..100 {
            println!(
                "{}",
                <$t>::generate(&mut v, &mut gen_depth.clone(), &mut 0)
                    .data
                    .iter()
                    .map(|i| format!("{}\n", i))
                    .collect::<String>()
            );
            println!("--------------------------------");
        }
    };
}

#[macro_export]
macro_rules! impl_converter {
    ($t:ty) => {
        #[derive(Clone)]
        struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            fn new() -> Self {
                Self {}
            }
        }

        impl libafl::inputs::TargetBytesConverter for FuzzDataTargetBytesConverter {
            type Input = $t;

            fn to_target_bytes<'a>(
                &mut self,
                input: &'a Self::Input,
            ) -> libafl_bolts::ownedref::OwnedSlice<'a, u8> {
                let bytes = thesis::serialize(&input);
                libafl_bolts::ownedref::OwnedSlice::from(bytes)
            }
        }
    };
    ($t:ty, $closure:expr) => {
        #[derive(Clone)]
        struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            fn new() -> Self {
                Self
            }
        }

        impl libafl::inputs::TargetBytesConverter for FuzzDataTargetBytesConverter {
            type Input = $t;

            fn to_target_bytes<'a>(
                &mut self,
                input: &'a Self::Input,
            ) -> libafl_bolts::ownedref::OwnedSlice<'a, u8> {
                libafl_bolts::ownedref::OwnedSlice::from($closure(input.clone()))
            }
        }
    };
}

#[macro_export]
macro_rules! impl_input {
    ($t:ty) => {
        impl libafl::inputs::Input for $t {
            fn to_file<P>(&self, path: P) -> Result<(), libafl::Error>
            where
                P: AsRef<std::path::Path>,
            {
                let bytes = thesis::serialize(self);
                std::fs::write(path, bytes)?;
                Ok(())
            }
            // TODO: don't serialize here
            fn generate_name(&self, id: Option<libafl::corpus::CorpusId>) -> String {
                let bytes = thesis::serialize(self);
                format!("{}", blake3::hash(bytes.as_slice()))
            }

            fn from_file<P>(path: P) -> Result<Self, libafl::Error>
            where
                P: AsRef<std::path::Path>,
            {
                let data = std::fs::read(path)?;
                let res = thesis::deserialize::<$t>(&mut data.as_slice());
                Ok(res)
            }
        }
    };
}
