#![allow(warnings)]

#[cfg(feature = "autarkie_derive")]
pub use autarkie_derive::Grammar;

pub use libafl::corpus::CorpusId;
pub use libafl::executors::ExitKind as LibAFLExitKind;
pub use libafl::inputs::Input;
pub use libafl::inputs::ToTargetBytes;
pub use libafl::Error as LibAFLError;
pub use libafl_bolts::ownedref::OwnedSlice;

pub mod tree;
pub mod visitor;
pub use tree::*;
pub use visitor::*;

mod graph;

#[cfg(feature = "scale")]
pub mod scale;

#[cfg(not(feature = "scale"))]
pub mod serde;
#[cfg(not(feature = "scale"))]
pub use serde::*;

pub mod fuzzer;
pub use fuzzer::afl;
pub use fuzzer::libfuzzer;

pub use blake3::hash;
