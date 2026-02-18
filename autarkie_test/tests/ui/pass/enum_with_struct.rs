use autarkie::Grammar;
use serde::{Deserialize, Serialize};

/// Enum variant containing a derived struct
#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub struct Payload {
    data: Vec<u8>,
    len: u32,
}

#[derive(Clone, Debug, Grammar, Serialize, Deserialize)]
pub enum Message {
    Empty {},
    Text(String),
    Binary(Payload),
    Pair(u64, String),
}

fn main() {}
