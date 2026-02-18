use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum UnitEnum {
    Alpha {},
    Beta {},
    Gamma {},
}

fn main() {}
