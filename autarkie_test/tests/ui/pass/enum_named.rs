use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum NamedEnum {
    Alpha { x: u64, y: String },
    Beta { flag: bool },
}

fn main() {}
