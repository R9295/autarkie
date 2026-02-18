use autarkie::Grammar;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum UnnamedEnum {
    One(u64),
    Two(String, bool),
    Three(u8, u16, u32),
}

fn main() {}
