use autarkie::ToTargetBytes;
use core::ffi::c_int;
use grammar_source::{FuzzData, FuzzDataTargetBytesConverter};
use libafl::executors::ExitKind;
use libafl::Error;
use libafl_bolts::AsSlice;
fn fuzz_many_forking(harness: &extern "C" fn(*const u8, usize) -> c_int) -> Result<(), Error> {
    let harness = |input: &FuzzData| {
        let target = FuzzDataTargetBytesConverter::new().to_target_bytes(input);
        let buf = target.as_slice();
        let result = unsafe {
            crate::libafl_libfuzzer_test_one_input(Some(*harness), buf.as_ptr(), buf.len())
        };
        match result {
            -2 => ExitKind::Crash,
            _ => ExitKind::Ok,
        }
    };
    autarkie::fuzzer::run_fuzzer(FuzzDataTargetBytesConverter::new(), Some(harness));
    Ok(())
}

pub fn fuzz(harness: &extern "C" fn(*const u8, usize) -> c_int) -> Result<(), Error> {
    fuzz_many_forking(harness)
}
