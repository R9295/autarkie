#[macro_export]
macro_rules! fuzz_afl_inner {
    ($t: ty) => {
        fn main() {
            let harness: Option<fn(&$t) -> autarkie::LibAFLExitKind> = None;
            $crate::fuzzer::run_fuzzer(FuzzDataTargetBytesConverter::new(), harness, __autarkie_loader);
        }
    };
}

#[macro_export]
macro_rules! fuzz_afl {
    ($t:ty) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t);
        $crate::fuzz_afl_inner!($t);
        $crate::impl_hash!($t);
    };
    ($t:ty, $closure:expr) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t, $closure);
        $crate::fuzz_afl_inner!($t);
        $crate::impl_hash!($t);
    };
    ($t:ty, $closure:expr, $loader:expr) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t, $closure);
        $crate::fuzz_afl_inner!($t);
        $crate::impl_hash!($t);
        $crate::impl_loader!($loader);
    };
}
