use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct WithLiteral {
    #[autarkie_literal(42)]
    single: u64,
    #[autarkie_literal(1, 2, 3)]
    multi: u32,
    name: String,
}

fn main() {}
