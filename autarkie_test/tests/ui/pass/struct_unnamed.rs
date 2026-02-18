use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct UnnamedStruct(u64, String, bool);

fn main() {}
