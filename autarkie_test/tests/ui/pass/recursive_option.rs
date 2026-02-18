use autarkie::Grammar;
use serde::{Deserialize, Serialize};

/// Recursion through Option
#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum LinkedList {
    Nil {},
    Cons(u64, Box<Option<LinkedList>>),
}

fn main() {}
