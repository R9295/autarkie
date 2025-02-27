#![allow(warnings)]
#![feature(core_intrinsics)]

#[cfg(feature = "autarkie_derive")]
pub use autarkie_derive::Grammar;

pub use blake3::hash;
pub use libafl::corpus::CorpusId;
pub use libafl::inputs::Input;
pub use libafl::inputs::TargetBytesConverter;
pub use libafl_bolts::ownedref::OwnedSlice;
pub use libafl_bolts::Error as LibAFLError;

pub mod tree;
pub mod visitor;
pub use tree::*;
pub use visitor::*;

mod graph;

#[cfg(feature = "bincode")]
pub mod serde;
#[cfg(feature = "bincode")]
pub use serde::*;

#[cfg(feature = "scale")]
pub mod scale;
#[cfg(feature = "scale")]
pub use scale::*;

pub mod fuzzer;

#[macro_export]
macro_rules! impl_converter {
    ($t:ty) => {
        #[derive(Clone)]
        pub struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            fn new() -> Self {
                Self {}
            }
        }

        impl autarkie::TargetBytesConverter for FuzzDataTargetBytesConverter {
            type Input = $t;

            fn to_target_bytes<'a>(
                &mut self,
                input: &'a Self::Input,
            ) -> autarkie::OwnedSlice<'a, u8> {
                let bytes = autarkie::serialize(&input);
                autarkie::OwnedSlice::from(bytes)
            }
        }
    };
    // We may want to render to bytes manually (eg: to_string) so we offer the possibility of a closure too.
    ($t:ty, $closure:expr) => {
        #[derive(Clone)]
        struct FuzzDataTargetBytesConverter;

        impl FuzzDataTargetBytesConverter {
            fn new() -> Self {
                Self
            }
        }

        impl autarkie::TargetBytesConverter for FuzzDataTargetBytesConverter {
            type Input = $t;

            fn to_target_bytes<'a>(
                &mut self,
                input: &'a Self::Input,
            ) -> autarkie::OwnedSlice<'a, u8> {
                autarkie::OwnedSlice::from($closure(input.clone()))
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
macro_rules! fuzz {
    ($t:ty) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t);
    };
    ($t:ty, $closure:expr) => {
        $crate::impl_input!($t);
        $crate::impl_converter!($t, $closure);
    };
}
