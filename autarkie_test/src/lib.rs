#[allow(unused_imports)]
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
    What(Box<Option<Expr>>),
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
pub struct Inner {
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

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct ShadowedMacroLocals {
    v: u32,
    val: u32,
    depth: u32,
    cur_depth: u32,
    is_recursive: u32,
    autarkie_visitor: u32,
    autarkie_path: u32,
    __autarkie_val: u32,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum ShadowedMacroLocalEnum {
    Named {
        v: u32,
        val: u32,
        depth: u32,
        cur_depth: u32,
        is_recursive: u32,
        autarkie_visitor: u32,
        autarkie_path: u32,
        __autarkie_val: u32,
    },
    Tuple(u32, u32),
}

#[derive(Clone, Debug, Grammar, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeightedChoice {
    #[autarkie_weight(0)]
    Disabled,
    Enabled,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum MutualA {
    Leaf(u32),
    Next(Box<MutualB>),
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum MutualB {
    Leaf(u32),
    Next(Box<MutualA>),
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct RangedFields {
    #[autarkie_range(10..=10)]
    fixed_ten: u8,
    #[autarkie_range(0..=0)]
    always_zero: u32,
    #[autarkie_length(3)]
    three_items: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use autarkie::{DepthInfo, Visitor};

    use super::*;

    #[test]
    fn derive_allows_fields_named_like_macro_locals() {
        let _ = ShadowedMacroLocals {
            v: 1,
            val: 2,
            depth: 3,
            cur_depth: 4,
            is_recursive: 5,
            autarkie_visitor: 6,
            autarkie_path: 7,
            __autarkie_val: 8,
        };
        let _ = ShadowedMacroLocalEnum::Named {
            v: 1,
            val: 2,
            depth: 3,
            cur_depth: 4,
            is_recursive: 5,
            autarkie_visitor: 6,
            autarkie_path: 7,
            __autarkie_val: 8,
        };
    }

    #[test]
    fn register_ty() {
        let mut visitor = Visitor::new(
            0,
            DepthInfo {
                generate: 2,
                iterate: 2,
            },
            0,
        );
        Statement::__autarkie_register(&mut visitor, None, 0);
        let recursion_by_name = visitor
            .calculate_recursion()
            .into_iter()
            .map(|(id, variants)| {
                (
                    visitor
                        .ty_name_map()
                        .get(&id)
                        .expect("registered recursive type")
                        .clone(),
                    variants,
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            recursion_by_name,
            BTreeMap::from_iter([
                (
                    "autarkie_test::Expr".to_string(),
                    BTreeSet::from_iter([2, 3, 4, 5, 6, 7, 8, 9])
                ),
                (
                    "core::result::Result<autarkie_test::InnerBoxed, usize>".to_string(),
                    BTreeSet::from_iter([0])
                )
            ])
        );
    }

    #[test]
    fn mutual_recursion_equal_variant_count_is_detected() {
        let mut visitor = Visitor::new(
            0,
            DepthInfo {
                generate: 2,
                iterate: 2,
            },
            0,
        );
        MutualA::__autarkie_register(&mut visitor, None, 0);
        let recursive = visitor.calculate_recursion();
        let recursion_by_name = recursive
            .into_iter()
            .map(|(id, variants)| {
                (
                    visitor.ty_name_map().get(&id).cloned().unwrap_or_default(),
                    variants,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let a_marked = recursion_by_name
            .get("autarkie_test::MutualA")
            .map_or(false, |v| v.contains(&1));
        let b_marked = recursion_by_name
            .get("autarkie_test::MutualB")
            .map_or(false, |v| v.contains(&1));
        assert!(
            a_marked || b_marked,
            "expected mutual recursion to be detected, got {recursion_by_name:?}"
        );

        for _ in 0..64 {
            let _ = MutualA::__autarkie_generate(&mut visitor, &mut 2, 0, None);
        }
    }

    #[test]
    fn autarkie_range_and_length_hints_are_honored() {
        let mut visitor = Visitor::new(
            12345,
            DepthInfo {
                generate: 2,
                iterate: 5,
            },
            0,
        );
        RangedFields::__autarkie_register(&mut visitor, None, 0);
        visitor.calculate_recursion();
        for _ in 0..256 {
            let generated =
                RangedFields::__autarkie_generate(&mut visitor, &mut 2, 0, None).expect("generated");
            assert_eq!(generated.fixed_ten, 10, "range 10..=10 must yield 10");
            assert_eq!(generated.always_zero, 0, "range 0..=0 must not panic and yields 0");
            assert_eq!(
                generated.three_items.len(),
                3,
                "autarkie_length(3) must yield exactly 3 elements"
            );
        }
    }

    #[test]
    fn recursive_replace_only_generates_non_recursive_variants() {
        let mut visitor = Visitor::new(
            7,
            DepthInfo {
                generate: 3,
                iterate: 3,
            },
            8,
        );
        Expr::__autarkie_register(&mut visitor, None, 0);
        visitor.calculate_recursion();
        for _ in 0..512 {
            let generated = visitor.with_non_recursive(|v| {
                Expr::__autarkie_generate(v, &mut 3, 0, None)
            });
            if let Some(expr) = generated {
                assert!(
                    matches!(expr, Expr::Literal(_) | Expr::Number(_)),
                    "force_non_recursive picked a recursive variant: {expr:?}"
                );
            }
        }
    }

    #[test]
    fn autarkie_weight_zero_disables_generation_variant() {
        let mut visitor = Visitor::new(
            0,
            DepthInfo {
                generate: 2,
                iterate: 2,
            },
            0,
        );
        WeightedChoice::__autarkie_register(&mut visitor, None, 0);
        visitor.calculate_recursion();

        for _ in 0..32 {
            let generated = WeightedChoice::__autarkie_generate(&mut visitor, &mut 2, 0, None);
            assert_eq!(generated, Some(WeightedChoice::Enabled));
        }
    }
}
