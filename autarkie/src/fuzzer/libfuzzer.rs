#[macro_export]
macro_rules! libfuzzer_main {
    ($t: ty) => {
        fn main() {}
    };
}

#[macro_export]
macro_rules! fuzz_libfuzzer {
    ($t:ty) => {
        $crate::impl_input!($t);
        $crate::libfuzzer_main!($t);
    };
    ($t:ty, $closure:expr) => {
        $crate::impl_input!($t);
        $crate::libfuzzer_main!($t);
    };
}
