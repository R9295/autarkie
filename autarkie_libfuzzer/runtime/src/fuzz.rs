use autarkie::ToTargetBytes;
use core::ffi::c_int;
use grammar_source::{FuzzData, FuzzDataTargetBytesConverter};
use libafl::executors::ExitKind;
use libafl::Error;
use libafl_bolts::AsSlice;
use std::path::PathBuf;

fn run_path_from_args() -> Option<PathBuf> {
    let mut args = std::env::args_os();
    while let Some(arg) = args.next() {
        if arg == "--run" {
            return args.next().map(PathBuf::from);
        }

        if let Some(arg) = arg.to_str() {
            if let Some(path) = arg.strip_prefix("--run=") {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}

fn run_file_libfuzzer(
    harness: extern "C" fn(*const u8, usize) -> c_int,
    input: PathBuf,
) -> Result<(), Error> {
    let mut files = if input.is_dir() {
        input
            .read_dir()
            .map_err(|err| Error::os_error(err, format!("Unable to read {}", input.display())))?
            .filter_map(core::result::Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>()
    } else {
        vec![input]
    };
    files.sort();

    for file in &files {
        eprintln!("\x1b[33mRunning: {}\x1b[0m", file.display());
        let input = std::fs::read(file)
            .map_err(|err| Error::os_error(err, format!("Unable to read {}", file.display())))?;
        let result = unsafe {
            crate::libafl_libfuzzer_test_one_input(Some(harness), input.as_ptr(), input.len())
        };
        if result == -2 {
            return Err(Error::unknown(format!(
                "Target threw an exception while running {}",
                file.display()
            )));
        }
    }

    Ok(())
}

fn fuzz_many_forking(harness: &extern "C" fn(*const u8, usize) -> c_int) -> Result<(), Error> {
    if let Some(input) = run_path_from_args() {
        return run_file_libfuzzer(*harness, input);
    }

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
