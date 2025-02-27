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

    // early duplicate the stderr fd so we can close it later for the target
    #[cfg(unix)]
    {
        use std::{
            os::fd::{AsRawFd, FromRawFd},
            str::FromStr,
        };

        let stderr_fd = std::env::var(STDERR_FD_VAR)
            .map_err(Error::from)
            .and_then(|s| RawFd::from_str(&s).map_err(Error::from))
            .unwrap_or_else(|_| {
                let stderr = unsafe { libc::dup(stderr().as_raw_fd()) };
                unsafe {
                    std::env::set_var(STDERR_FD_VAR, stderr.to_string());
                }
                stderr
            });
        let stderr = unsafe { File::from_raw_fd(stderr_fd) };
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

    let argc = unsafe { *argc } as isize;
    let argv = unsafe { *argv };

    // TODO: fuzz
}
