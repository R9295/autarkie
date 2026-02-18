use autarkie::Grammar;
use serde::{Deserialize, Serialize};

/// Recursion through Vec (iterable recursion)
#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Tree {
    Leaf(u64),
    Children(Vec<Tree>),
}

fn main() {}
