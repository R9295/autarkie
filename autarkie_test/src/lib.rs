use autarkie::{Grammar, Node};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Expr {
    Literal(String),
    Number(u128),
    Add(Box<Expr>, Box<Expr>),
    Vec(Vec<Expr>),
    What(Box<Option<Expr>>),
    WhatTwo(InnerBoxed),
    WhatTwoInner(InnerBoxedEnum),
    SayWhat((usize, Box<Expr>)),
    Res(Result<InnerBoxed, usize>),
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct  Inner {
    what: Expr,
    who: u64,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct InnerBoxed {
    what: Box<Expr>,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum InnerBoxedEnum {
    Test(Box<Expr>),
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Statement {
    Exp(Expr),
}
#[cfg(test)]
mod tests {
    use autarkie::Visitor;

    use super::*;
    #[test]
    fn register_ty() {
        let mut visitor = Visitor::new(
            0,
            autarkie::DepthInfo {
                generate: 2,
                iterate: 2,
            },
        );
        Statement::__autarkie_register(&mut visitor, None, 0);
        visitor.print_ty();
    }
}

// tuple time.
// TODO: Option must be a special type.
