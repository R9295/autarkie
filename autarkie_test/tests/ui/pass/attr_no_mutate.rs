use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct Config {
    #[autarkie_no_mutate]
    version: u64,
    data: String,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Command {
    Set(#[autarkie_no_mutate] u64, String),
    Get { #[autarkie_no_mutate] id: u32, query: String },
}

fn main() {}
