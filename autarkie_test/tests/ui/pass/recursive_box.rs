use autarkie::Grammar;
use serde::{Deserialize, Serialize};

/// Direct self-recursion via Box
#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Expr {
    Literal(u64),
    Add(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
}

fn main() {}
