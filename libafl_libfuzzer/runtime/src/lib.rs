use core::ffi::{c_char, c_int};

use env_logger::Target;
use libafl::Error;
use mimalloc::MiMalloc;
use std::os::fd::RawFd;
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

pub(crate) use harness_wrap::libafl_libfuzzer_test_one_input;
unsafe extern "C" {
    // redeclaration against libafl_targets because the pointers in our case may be mutable
    fn libafl_targets_libfuzzer_init(argc: *mut c_int, argv: *mut *mut *const c_char) -> i32;
}

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
    // early duplicate the stderr fd so we can close it later for the target
    #[cfg(unix)]
    {
        use std::{
            os::fd::{AsRawFd, FromRawFd},
            str::FromStr,
        };

        let stderr_fd = std::env::var(autarkie::fuzzer::STDERR_FD_VAR)
            .map_err(Error::from)
            .and_then(|s| RawFd::from_str(&s).map_err(Error::from))
            .unwrap_or_else(|_| {
                let stderr = unsafe { libc::dup(std::io::stderr().as_raw_fd()) };
                unsafe {
                    std::env::set_var(autarkie::fuzzer::STDERR_FD_VAR, stderr.to_string());
                }
                stderr
            });
        let stderr = unsafe { std::fs::File::from_raw_fd(stderr_fd) };
        env_logger::builder()
            .parse_default_env()
            .target(Target::Pipe(Box::new(stderr)))
            .init();
    }

    // it appears that no one, not even libfuzzer, uses this return value
    // https://github.com/llvm/llvm-project/blob/llvmorg-15.0.7/compiler-rt/lib/fuzzer/FuzzerDriver.cpp#L648
    unsafe {
        libafl_targets_libfuzzer_init(argc, argv);
    }
    let res = crate::fuzz::fuzz(harness);
    match res {
        Ok(()) | Err(Error::ShuttingDown) => 0,
        Err(err) => {
            eprintln!("Encountered error while performing libfuzzer shimming: {err}");
            1
        }
    }
}
