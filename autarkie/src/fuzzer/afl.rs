#[macro_export]
macro_rules! impl_converter {
    ($t:ty) => {
        #[derive(Clone)]
        pub struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            pub fn new() -> Self {
                Self {}
            }
        }

        impl<I: autarkie::Node> autarkie::TargetBytesConverter<I> for FuzzDataTargetBytesConverter {
            fn to_target_bytes<'a>(&mut self, input: &'a I) -> autarkie::OwnedSlice<'a, u8> {
                let bytes = autarkie::serialize(input);
                autarkie::OwnedSlice::from(bytes)
            }
        }
    };
    // We may want to render to bytes manually (eg: to_string) so we offer the possibility of a closure too.
    ($t:ty, $closure:expr) => {
        #[derive(Clone)]
        pub struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            pub fn new() -> Self {
                Self
            }
        }

        impl autarkie::TargetBytesConverter<$t> for FuzzDataTargetBytesConverter {
            fn to_target_bytes<'a>(&mut self, input: &'a $t) -> autarkie::OwnedSlice<'a, u8> {
                autarkie::OwnedSlice::from($closure(input))
            }
        }
    };
}

#[macro_export]
macro_rules! impl_input {
    ($t:ty) => {
        impl autarkie::Input for $t {
            fn to_file<P>(&self, path: P) -> Result<(), autarkie::LibAFLError>
            where
                P: AsRef<std::path::Path>,
            {
                let bytes = autarkie::serialize(self);
                std::fs::write(path, bytes)?;
                Ok(())
            }

            // TODO: don't serialize here
            fn generate_name(&self, id: Option<autarkie::CorpusId>) -> String {
                let bytes = autarkie::serialize(self);
                std::format!("{}", autarkie::hash(bytes.as_slice()))
            }

            fn from_file<P>(path: P) -> Result<Self, autarkie::LibAFLError>
            where
                P: AsRef<std::path::Path>,
            {
                let data = std::fs::read(path)?;
                let res = autarkie::deserialize::<$t>(&mut data.as_slice());
                Ok(res)
            }
        }
    };
}

#[macro_export]
macro_rules! fuzz_afl_inner {
    ($t: ty) => {
        fn main() {
            $crate::fuzzer::run_fuzzer(FuzzDataTargetBytesConverter::new(), None);
        }
    };
}

#[macro_export]
macro_rules! fuzz_afl {
    ($t:ty) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t);
        $crate::fuzz_afl_inner!($t);
    };
    ($t:ty, $closure:expr) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t, $closure);
        $crate::fuzz_afl_inner!($t);
    };
}

#[macro_export]
macro_rules! impl_hash {
    ($t:ty) => {
        impl std::hash::Hash for $t {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                autarkie::serialize(&self).hash(state)
            }
        }
    };
}
