use autarkie::Grammar;
use serde::{Deserialize, Serialize};

/// Indirect recursion: Statement -> Expr -> Statement
#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Expr {
    Literal(u64),
    Block(Box<Statement>),
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Statement {
    Expr(Expr),
    Return(Box<Expr>),
}

fn main() {}
