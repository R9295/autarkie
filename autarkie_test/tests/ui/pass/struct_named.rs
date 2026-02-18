use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct NamedStruct {
    x: u64,
    y: String,
    z: bool,
}

fn main() {}
