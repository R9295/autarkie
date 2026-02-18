use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct WithRange {
    #[autarkie_range(0..=100)]
    bounded: u64,
    other: bool,
}

fn main() {}
