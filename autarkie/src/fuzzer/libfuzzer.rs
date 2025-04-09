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
