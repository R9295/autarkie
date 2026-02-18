use autarkie::Grammar;
use serde::{Deserialize, Serialize};

/// Complex recursive tree matching the pattern in autarkie_test/src/lib.rs
#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Expr {
    Literal(String),
    Number(u128),
    Add(Box<Expr>, Box<Expr>),
    List(Vec<Expr>),
    Maybe(Box<Option<Expr>>),
    Indirect(Wrapper),
    Stmt(Box<Statement>),
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct Wrapper {
    inner: WrapperInner,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct WrapperInner {
    expr: Box<Expr>,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Statement {
    Expr(Expr),
}

fn main() {}
