use autarkie::{Grammar, Node};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Expr {
    Literal(String),
    Number(u128),
    // recursive
    Add(Box<Expr>, Box<Expr>),
    // potentially recursive
    Vec(Vec<Expr>),
    // potentially recursive
    What(
        Box<
            Option<Expr>
        >
    ),
    // TODO 5 recursive
    WhatTwo(InnerBoxed),
    // recursive
    WhatTwoInner(InnerBoxedEnum),
    // recursive
    SayWhat((usize, Box<Expr>)),
    // TODO: 8 potentially recursive
    Res(Result<InnerBoxed, usize>),
    // recursive
    Stmt(Box<Statement>),
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct  Inner {
    what: Expr,
    who: u64,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct InnerBoxed {
    what: InnerInnerBoxed,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct InnerInnerBoxed {
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
    use std::collections::{BTreeMap, BTreeSet};

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
        assert_eq!(visitor.calculate_recursion() , BTreeMap::from_iter(
            [("autarkie_test::Expr".to_string(), BTreeSet::from_iter([2,3,4,5,6,7,8,9])),
            ("core::option::Option<autarkie_test::Expr>".to_string(), BTreeSet::from_iter([1])),
            ("core::result::Result<autarkie_test::InnerBoxed, usize>".to_string(), BTreeSet::from_iter([0]))]
        ));
    }
}

// tuple time.
// TODO: Option must be a special type.
