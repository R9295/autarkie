use super::context::{self, MutationMetadata};
use super::feedback::register::RegisterFeedback;
use super::mutators::iterable_pop::AutarkieIterablePopMutator;
use super::mutators::vecu8::AutarkieVecU8Mutator;
use crate::fuzzer::context::Context;
#[cfg(feature = "afl")]
use crate::fuzzer::stages::cmp::CmpLogStage;
use crate::fuzzer::stages::generate::generate;
use crate::{DepthInfo, Visitor};
use clap::Parser;
use libafl::executors::forkserver::SHM_CMPLOG_ENV_VAR;
use libafl::executors::StdChildArgs;
use libafl::monitors::SimpleMonitor;
#[cfg(feature = "libfuzzer")]
use libafl::stages::ShadowTracingStage;
use libafl::stages::{SyncFromDiskStage, TracingStage};
use libafl::{events::LlmpRestartingEventManager, mutators::I2SRandReplace};
use libafl_bolts::StdTargetArgs;

use crate::fuzzer::mutators::{
    generate_append::AutarkieGenerateAppendMutator,
    recurse_mutate::{AutarkieRecurseMutator, RECURSE_STACK},
    splice::{AutarkieSpliceMutator, SPLICE_STACK},
    splice_append::{AutarkieSpliceAppendMutator, SPLICE_APPEND_STACK},
};
use crate::fuzzer::stages::{
    binary_mutator::AutarkieBinaryMutatorStage,
    generate::GenerateStage,
    minimization::MinimizationStage,
    mutating::MutatingStageWrapper,
    mutational::AutarkieMutationalStage,
    novelty_minimization::NoveltyMinimizationStage,
    recursive_minimization::RecursiveMinimizationStage,
    stats::{AutarkieStats, StatsStage},
};
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
    inputs::{BytesInput, HasTargetBytes, InputConverter, ToTargetBytes},
    monitors::MultiMonitor,
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
use libafl_bolts::AsSlice;
use libafl_bolts::{
    core_affinity::{CoreId, Cores},
    fs::get_unique_std_input_file,
    ownedref::OwnedRefMut,
    rands::{RomuDuoJrRand, StdRand},
    shmem::{ShMem, ShMemProvider, StdShMemProvider, UnixShMemProvider},
    tuples::{tuple_list, Handled},
    AsSliceMut, Error,
};
use libafl_bolts::{shmem::StdShMem, tuples::Merge};
#[cfg(feature = "libfuzzer")]
use libafl_targets::{extra_counters, CmpLogObserver};
#[cfg(feature = "afl")]
use libafl_targets::{AflppCmpLogMap, AflppCmpLogObserver, AflppCmplogTracingStage};
use regex::Regex;

use crate::fuzzer::hooks::rare_share::RareShare;
use std::io::{stderr, stdout, Write};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::str::FromStr;
use std::{cell::RefCell, io::ErrorKind, path::PathBuf, process::Command, rc::Rc, time::Duration};
use std::{env::args, ffi::c_int};

use crate::{Input, Node};
pub type AutarkieState<I> = StdState<CachedOnDiskCorpus<I>, I, StdRand, OnDiskCorpus<I>>;

#[cfg(not(feature = "fuzzbench"))]
type AutarkieManager<I> =
    LlmpRestartingEventManager<(), I, AutarkieState<I>, StdShMem, StdShMemProvider>;
#[cfg(feature = "fuzzbench")]
type AutarkieManager<F, I> = SimpleEventManager<I, SimpleMonitor<F>, AutarkieState<I>>;

macro_rules! define_run_client {
    ($state: ident, $mgr: ident, $core: ident, $bytes_converter: ident, $opt: ident, $body:block) => {
        #[cfg(not(feature = "fuzzbench"))]
        pub fn run_client<I: Node + Input, TC: ToTargetBytes<I> + Clone>(
            $state: Option<AutarkieState<I>>,
            mut $mgr: AutarkieManager<I>,
            $core: ClientDescription,
            $bytes_converter: TC,
            $opt: &super::Opt,
        ) -> Result<(), Error> {
            $body
        }
        #[cfg(feature = "fuzzbench")]
        pub fn run_client<F, I: Node + Input, TC: ToTargetBytes<I> + Clone>(
            $state: Option<AutarkieState<I>>,
            mut $mgr: AutarkieManager<F, I>,
            $core: ClientDescription,
            $bytes_converter: TC,
            $opt: &super::Opt,
        ) -> Result<(), Error>
        where
            F: FnMut(&str),
        {
            $body
        }
    };
}

define_run_client!(state, mgr, core, bytes_converter, opt, {
    let is_main_node = opt.cores.position(core.core_id()).unwrap() == 0;
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
    /* #[cfg(feature = "libfuzzer")]
    let cmplog_observer = CmpLogObserver::new("cmplog", true); */
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
    let edges_observer = StdMapObserver::from_mut_slice("edges", edges.into_iter().next().unwrap())
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
        opt.string_pool_size,
    );
    I::__autarkie_register(&mut visitor, None, 0);
    let recursive_nodes = visitor.calculate_recursion();
    if is_main_node {
        std::fs::write(
            opt.output_dir.join("type_input_map.json"),
            serde_json::to_string_pretty(visitor.ty_name_map()).expect("invariant"),
        )?;
    }
    let has_recursion = recursive_nodes.len() > 0;
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
        tuple_list!(
            NoveltyMinimizationStage::new(Rc::clone(&visitor), &map_feedback),
            MinimizationStage::new(Rc::clone(&visitor), &map_feedback),
            RecursiveMinimizationStage::new(Rc::clone(&visitor), &map_feedback),
        ),
    );
    let cb = |_fuzzer: &mut _,
              _executor: &mut _,
              state: &mut StdState<CachedOnDiskCorpus<I>, I, StdRand, OnDiskCorpus<I>>,
              _event_manager: &mut _|
     -> Result<bool, Error> {
        Ok(state.current_testcase_mut()?.scheduled_count() == 0)
    };

    let minimization_stage = IfStage::new(cb, tuple_list!(novelty_minimization_stage,));
    let mut feedback = feedback_or!(
        map_feedback,
        TimeFeedback::new(&time_observer),
        RegisterFeedback::new(Rc::clone(&visitor), bytes_converter.clone(), false),
    );

    let mut objective = feedback_or_fast!(
        CrashFeedback::new(),
        TimeoutFeedback::new(),
        RegisterFeedback::new(Rc::clone(&visitor), bytes_converter.clone(), true),
    );

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
    if !fuzzer_dir.join("rendered_corpus").exists() {
        std::fs::create_dir(fuzzer_dir.join("rendered_corpus")).unwrap();
    }
    if !fuzzer_dir.join("rendered_crashes").exists() {
        std::fs::create_dir(fuzzer_dir.join("rendered_crashes")).unwrap();
    }
    if !fuzzer_dir.join("cmps").exists() {
        std::fs::create_dir(fuzzer_dir.join("cmps")).unwrap();
    }

    let mut context = Context::new(fuzzer_dir.clone(), opt.render);

    /* let scheduler = StdWeightedScheduler::with_schedule(
        &mut state,
        &edges_observer,
        Some(PowerSchedule::explore()),
    ); */
    let observers = tuple_list!(time_observer);
    let scheduler = QueueScheduler::new(); //scheduler.cycling_scheduler();
                                           // Create our Fuzzer
                                           /*     let mut filter = BloomInputFilter::new(5000, 0.0001); */
    let mut fuzzer = StdFuzzerBuilder::new()
        /*         .input_filter(filter) */
        .target_bytes_converter(bytes_converter.clone())
        .scheduler(scheduler)
        .feedback(feedback)
        .objective(objective)
        .build();

    // Create our Executor
    #[cfg(feature = "afl")]
    let mut executor = ForkserverExecutor::builder()
        .program(opt.executable.clone())
        .coverage_map_size(map_size)
        .debug_child(opt.debug_child)
        .is_persistent(true)
        .is_deferred_frksrv(true)
        .timeout(Duration::from_millis(opt.hang_timeout * 1000))
        .shmem_provider(&mut shmem_provider)
        .build_dynamic_map(edges_observer, observers)
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
        Duration::from_millis(opt.hang_timeout * 1000),
    )?;
    /* #[cfg(feature = "libfuzzer")]
    let mut executor = ShadowExecutor::new(executor, tuple_list!(cmplog_observer)); */
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
    let mut gen = vec![];
    // Reload corpus
    if state.must_load_initial_inputs() {
        state.load_initial_inputs(
            &mut fuzzer,
            &mut executor,
            &mut mgr,
            &[fuzzer_dir.join("queue").clone(), fuzzer_dir.join("crash")],
        )?;
        for _ in 0..opt.initial_generated_inputs {
            let mut metadata = state.metadata_mut::<Context>().expect("fxeZamEw____");
            metadata.generated_input();
            let mut generated = generate(&mut visitor.borrow_mut());
            while generated.is_none() {
                generated = generate(&mut visitor.borrow_mut());
            }
            gen.push(generated.clone().unwrap());
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
    // The cmplog map shared between observer and executor
    let mut cmplog_shmem = shmem_provider.uninit_on_shmem::<AflppCmpLogMap>().unwrap();
    // let the forkserver know the shmid
    unsafe {
        cmplog_shmem.write_to_env(SHM_CMPLOG_ENV_VAR).unwrap();
    }
    let cmpmap = unsafe { OwnedRefMut::from_shmem(&mut cmplog_shmem) };
    #[cfg(feature = "afl")]
    let mut cmplog = {
        let cmplog_observer = AflppCmpLogObserver::new("cmplog", cmpmap, true);
        let cmplog_ref = cmplog_observer.handle();
        let mut cmplog_executor = ForkserverExecutor::builder()
            .program(opt.executable.clone())
            .coverage_map_size(65_536)
            .is_persistent(true)
            .timeout(Duration::from_millis(opt.hang_timeout * 1000) * 2)
            .shmem_provider(&mut shmem_provider)
            .build(tuple_list!(cmplog_observer))
            .unwrap();
        let tracing = CmpLogStage::new(Rc::clone(&visitor), cmplog_executor, cmplog_ref);
        let cmplog = IfStage::new(cb, tuple_list!(tracing));
        cmplog
    };

    let cb = |_fuzzer: &mut _, _executor: &mut _, path: &Path| -> Result<I, Error> {
        let data = std::fs::read(path)?;
        let Some(input) = crate::maybe_deserialize(&data) else {
            return Err(Error::invalid_input("Invalid structure"));
        };
        Ok(input)
    };
    let sync_stage = SyncFromDiskStage::new(
        opt.foreign_sync_dirs.clone(),
        cb,
        Duration::from_secs(15),
        "Sync",
    );
    let cb = |_fuzzer: &mut _,
              _executor: &mut _,
              state: &mut StdState<CachedOnDiskCorpus<I>, I, StdRand, OnDiskCorpus<I>>,
              _event_manager: &mut _|
     -> Result<bool, Error> { Ok(is_main_node) };
    let sync_stage = IfStage::new(cb, tuple_list!(sync_stage));
    let splice_mutator = AutarkieSpliceMutator::new(Rc::clone(&visitor), opt.max_subslice_size);
    let recursion_mutator = AutarkieRecurseMutator::new(Rc::clone(&visitor), opt.max_subslice_size);
    let splice_append_mutator = AutarkieSpliceAppendMutator::new(Rc::clone(&visitor));
    #[cfg(feature = "libfuzzer")]
    let i2s = AutarkieBinaryMutatorStage::new(
        tuple_list!(I2SRandReplace::new()),
        7,
        MutationMetadata::I2S,
    );
    // TODO: I2S for AFL
    #[cfg(feature = "afl")]
    let mut stages = tuple_list!(
        minimization_stage,
        MutatingStageWrapper::new(cmplog, Rc::clone(&visitor)),
        MutatingStageWrapper::new(
            AutarkieMutationalStage::new(
                tuple_list!(splice_append_mutator, recursion_mutator, splice_mutator,),
                SPLICE_STACK
            ),
            Rc::clone(&visitor)
        ),
        MutatingStageWrapper::new(
            AutarkieMutationalStage::new(
                tuple_list!(AutarkieVecU8Mutator::new(Rc::clone(&visitor), 20)),
                10,
            ),
            Rc::clone(&visitor)
        ),
        StatsStage::new(fuzzer_dir),
        sync_stage,
    );
    #[cfg(feature = "libfuzzer")]
    let mut stages = tuple_list!(
        minimization_stage,
        tracing,
        MutatingStageWrapper::new(i2s, Rc::clone(&visitor)),
        AutarkieMutationalStage::new(
            tuple_list!(
                splice_append_mutator,
                generate_append_mutator,
                recursion_mutator,
                recursion_mutator_two,
                recursion_mutator_three,
                splice_mutator
            ),
            SPLICE_STACK
        ),
        MutatingStageWrapper::new(generate_stage, Rc::clone(&visitor)),
        StatsStage::new(fuzzer_dir),
    );
    let res = fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr);
    Err(Error::shutting_down())
});
#[cfg(feature = "afl")]
const SHMEM_ENV_VAR: &str = "__AFL_SHM_ID";
