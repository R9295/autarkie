#[macro_export]
macro_rules! fuzz_libfuzzer {
    ($t:ty) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t);
        $crate::impl_hash!($t);
    };
    ($t:ty, $closure:expr) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t, $closure);
        $crate::impl_hash!($t);
    };
}

#[macro_export]
macro_rules! fuzz_libfuzzer_link {
    ($t:ty) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t);
        $crate::fuzz_libfuzzer_link_inner!($t);
        $crate::impl_hash!($t);
    };
    ($t:ty, $closure:expr) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t, $closure);
        $crate::fuzz_libfuzzer_link_inner!($t);
        $crate::impl_hash!($t);
    };
}

#[macro_export]
macro_rules! fuzz_libfuzzer_link_inner {
    ($t: ty) => {
        fn main() {
            let args: Vec<String> = std::env::args().collect();
            if unsafe { autarkie::libfuzzer_initialize(&args) } == -1 {
                println!("Warning: LLVMFuzzerInitialize failed with -1");
            }
            use autarkie::ToTargetBytes;
            let mut harness = |input: &$t| {
                let target = FuzzDataTargetBytesConverter::new().to_target_bytes(&input);
                unsafe {
                    $crate::libfuzzer_test_one_input(&target);
                }
                $crate::LibAFLExitKind::Ok
            };
            $crate::fuzzer::run_fuzzer(FuzzDataTargetBytesConverter::new(), Some(harness));
        }
    };
}
