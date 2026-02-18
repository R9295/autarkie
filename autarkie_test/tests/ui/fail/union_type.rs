use autarkie::Grammar;

#[derive(Clone, Grammar)]
pub union BadUnion {
    x: u64,
    y: u32,
}

fn main() {}
