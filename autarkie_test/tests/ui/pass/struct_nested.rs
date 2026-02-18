use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct Inner {
    value: u32,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct Outer {
    inner: Inner,
    name: String,
}

fn main() {}
