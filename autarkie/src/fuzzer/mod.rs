#![allow(warnings)]
#![feature(core_intrinsics)]
mod fuzzer;
pub mod afl;
pub mod context;
mod feedback;
mod hooks;
pub mod libfuzzer;
pub mod mutators;
mod stages;

use clap::Parser;
use std::path::{PathBuf, Path};
use libafl_bolts::core_affinity::Cores;
use libafl::events::ClientDescription;
use libafl::events::{SimpleEventManager};
use libafl::monitors::MultiMonitor;
use libafl_bolts::shmem::StdShMemProvider;
use libafl::executors::ExitKind;
use crate::{Input, InputToBytes, Node};
use libafl_bolts::shmem::ShMemProvider;
use libafl_bolts::tuples::tuple_list;
use libafl::events::{EventConfig, Launcher};
use crate::fuzzer::hooks::rare_share::RareShare;

#[cfg(any(feature = "libfuzzer", feature = "afl"))]
pub fn run_fuzzer<I, TC, F>(bytes_converter: TC, harness: Option<F>)
where
    I: Node + Input,
    TC: InputToBytes<I> + Clone,
    F: Fn(&I) -> ExitKind,
{
    use libafl::monitors::SimpleMonitor;

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

    #[cfg(not(feature = "fuzzbench"))]
    Launcher::builder()
        .cores(&opt.cores)
        .monitor(monitor)
        .run_client(|s, mgr, core| {
            fuzzer::run_client(s, mgr, core, bytes_converter.clone(), &opt)
        })
        .broker_port(opt.broker_port)
        .shmem_provider(shmem_provider)
        .configuration(EventConfig::from_name("default"))
        .build()
        //.launch_with_hooks(tuple_list!(RareShare::new(opt.skip_count)));
        .launch_with_hooks(());
    #[cfg(feature = "fuzzbench")]
    {
        let monitor = SimpleMonitor::new(|s| println!("{}", s));
        let mgr = SimpleEventManager::new(monitor);
        fuzzer::run_client(None, mgr, ClientDescription::new(0, 0, 0.into()),bytes_converter.clone(), &opt);
    }
}

#[derive(Debug, Parser, Clone)]
#[command(
    name = "autarkie",
    about = "autarkie",
    author = "aarnav <aarnavbos@gmail.com>"
)]
pub(crate) struct Opt {
    /// What we wanna fuzz
    #[cfg(feature = "afl")]
    executable: PathBuf,
    /// Fuzzer output dir; will also load inputs from there
    #[arg(short = 'o')]
    output_dir: PathBuf,

    /// Timeout in seconds
    #[arg(short = 't', default_value_t = 1)]
    hang_timeout: u64,

    /// Share an entry only every n entries
    #[arg(short = 'K', default_value_t = 0)]
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

    /// string pool size
    #[arg(short = 'l', default_value_t = 50)]
    string_pool_size: usize,

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
                50,
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
