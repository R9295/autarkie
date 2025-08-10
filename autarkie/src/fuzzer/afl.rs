#[macro_export]
macro_rules! impl_converter {
    ($t:ty) => {
        #[derive(Debug, Clone)]
        pub struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            pub fn new() -> Self {
                Self {}
            }
        }

        impl autarkie::ToTargetBytes<$t> for FuzzDataTargetBytesConverter {
            fn to_target_bytes<'a>(&mut self, input: &'a $t) -> autarkie::OwnedSlice<'a, u8> {
                let bytes = autarkie::serialize(input);
                let bytes = if bytes.len() == 0 {
                    vec![0, 0, 0, 0]
                } else {
                    bytes
                };
                autarkie::OwnedSlice::from(bytes)
            }
        }
    };
    // We may want to render to bytes manually (eg: to_string) so we offer the possibility of a closure too.
    ($t:ty, $closure:expr) => {
        #[derive(Debug, Clone)]
        pub struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            pub fn new() -> Self {
                Self
            }
        }

        impl autarkie::ToTargetBytes<$t> for FuzzDataTargetBytesConverter {
            fn to_target_bytes<'a>(&mut self, input: &'a $t) -> autarkie::OwnedSlice<'a, u8> {
                let bytes = $closure(input);
                let bytes = if bytes.len() == 0 {
                    vec![0, 0, 0, 0]
                } else {
                    bytes
                };
                autarkie::OwnedSlice::from(bytes)
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
            let harness: Option<fn(&$t) -> autarkie::LibAFLExitKind> = None;
            $crate::fuzzer::run_fuzzer(FuzzDataTargetBytesConverter::new(), harness);
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
