use autarkie::Grammar;
use serde::{Deserialize, Serialize};

/// Enum mixing unit, unnamed, and named variants
#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum MixedEnum {
    Unit {},
    Unnamed(u64, String),
    Named { x: bool, y: u32 },
}

fn main() {}
