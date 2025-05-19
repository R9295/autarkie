#![allow(warnings)]
#![feature(core_intrinsics)]

#[cfg(feature = "autarkie_derive")]
pub use autarkie_derive::Grammar;

pub use blake3::hash;
pub use libafl::corpus::CorpusId;
pub use libafl::executors::ExitKind as LibAFLExitKind;
pub use libafl::inputs::Input;
pub use libafl::inputs::InputToBytes;
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
pub use fuzzer::afl;
pub use fuzzer::libfuzzer;
