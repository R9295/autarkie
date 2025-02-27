use core::ffi::{c_char, c_int};

pub use libfuzzer_sys::*;

unsafe extern "C" {
    pub fn LLVMFuzzerRunDriver(
        argc: *mut c_int,
        argv: *mut *mut *const c_char,
        harness_fn: Option<extern "C" fn(*const u8, usize) -> c_int>,
    ) -> c_int;
}

#[cfg(all(
    feature = "embed-runtime",
    target_family = "unix",
    // Disable when building with clippy, as it will complain about the missing environment
    // variable which is set by the build script, which is not run under clippy.
    not(clippy)
))]
pub const LIBAFL_LIBFUZZER_RUNTIME_LIBRARY: &'static [u8] =
    include_bytes!(env!("LIBAFL_LIBFUZZER_RUNTIME_PATH"));

