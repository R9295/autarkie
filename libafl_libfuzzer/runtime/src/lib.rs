use core::ffi::{CStr, c_char, c_int};
use std::{fs::File, io::stderr, os::fd::RawFd};

use env_logger::Target;
use libafl::Error;
use libafl_bolts::AsSlice;
use libc::_exit;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod fuzz;

mod harness_wrap {
    #![allow(non_snake_case)]
    #![allow(non_camel_case_types)]
    #![allow(non_upper_case_globals)]
    #![allow(unused)]
    #![allow(improper_ctypes)]
    #![allow(clippy::unreadable_literal)]
    #![allow(missing_docs)]
    #![allow(unused_qualifications)]
    include!(concat!(env!("OUT_DIR"), "/harness_wrap.rs"));
}

pub(crate) use harness_wrap::libafl_libfuzzer_test_one_input;

#[expect(clippy::struct_excessive_bools)]
struct CustomMutationStatus {
    std_mutational: bool,
    std_no_mutate: bool,
    std_no_crossover: bool,
    custom_mutation: bool,
    custom_crossover: bool,
}

impl CustomMutationStatus {
    fn new() -> Self {
        let custom_mutation = libafl_targets::libfuzzer::has_custom_mutator();
        let custom_crossover = libafl_targets::libfuzzer::has_custom_crossover();

        // we use all libafl mutations
        let std_mutational = !(custom_mutation || custom_crossover);
        // we use libafl crossover, but not libafl mutations
        let std_no_mutate = !std_mutational && custom_mutation && !custom_crossover;
        // we use libafl mutations, but not libafl crossover
        let std_no_crossover = !std_mutational && !custom_mutation && custom_crossover;

        Self {
            std_mutational,
            std_no_mutate,
            std_no_crossover,
            custom_mutation,
            custom_crossover,
        }
    }
}

/// Starts to fuzz on a single node
pub fn start_fuzzing_single<F, S, EM>(
    mut fuzz_single: F,
    initial_state: Option<S>,
    mgr: EM,
) -> Result<(), Error>
where
    F: FnMut(Option<S>, EM, usize) -> Result<(), Error>,
{
    fuzz_single(initial_state, mgr, 0)
}

unsafe extern "C" {
    // redeclaration against libafl_targets because the pointers in our case may be mutable
    fn libafl_targets_libfuzzer_init(argc: *mut c_int, argv: *mut *mut *const c_char) -> i32;
}

/// Communicate the stderr duplicated fd to subprocesses
pub const STDERR_FD_VAR: &str = "_LIBAFL_LIBFUZZER_STDERR_FD";

/// A method to start the fuzzer at a later point in time from a library.
/// To quote the `libfuzzer` docs:
/// > when it’s ready to start fuzzing, it can call `LLVMFuzzerRunDriver`, passing in the program arguments and a callback. This callback is invoked just like `LLVMFuzzerTestOneInput`, and has the same signature.
///
/// # Safety
/// Will dereference all parameters.
/// This will then call the (potentially unsafe) harness.
/// The fuzzer itself should catch any side effects and, hence be reasonably safe, if the `harness_fn` parameter is correct.
#[expect(clippy::similar_names)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn LLVMFuzzerRunDriver(
    argc: *mut c_int,
    argv: *mut *mut *const c_char,
    harness_fn: Option<extern "C" fn(*const u8, usize) -> c_int>,
) -> c_int {
    let harness = harness_fn
        .as_ref()
        .expect("Illegal harness provided to libafl.");

    // it appears that no one, not even libfuzzer, uses this return value
    // https://github.com/llvm/llvm-project/blob/llvmorg-15.0.7/compiler-rt/lib/fuzzer/FuzzerDriver.cpp#L648
    unsafe {
        libafl_targets_libfuzzer_init(argc, argv);
    }

    let argc = unsafe { *argc } as isize;
    let argv = unsafe { *argv };
    0
}
