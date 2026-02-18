use autarkie::Grammar;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct WithCollections {
    list: Vec<u32>,
    nested_list: Vec<Vec<u8>>,
    map: BTreeMap<u64, String>,
    tuple: (u8, u16, u32),
}

fn main() {}
