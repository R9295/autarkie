#![allow(warnings)]
#![feature(core_intrinsics)]

#[cfg(feature = "bincode")]
pub mod serde;
pub mod tree;
pub mod visitor;

#[cfg(feature = "autarkie_derive")]
pub use autarkie_derive::Grammar;

#[cfg(feature = "bincode")]
pub use serde::*;

pub use tree::*;
pub use visitor::*;

#[cfg(feature = "scale")]
pub mod scale;
#[cfg(feature = "scale")]
pub use scale::*;

