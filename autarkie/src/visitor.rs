use libafl_bolts::rands::{Rand, StdRand};

use crate::Id;

#[derive(Debug, Clone)]
pub enum NodeType {
    ///  A normal node
    NonRecursive,
    /// A node with a field which of type Self. eg: Box<Self>
    Recursive,
    /// An iterable node. eg Vec<T>
    Iterable(
        /// Amount of Elements
        usize,
        /// type id of Elements
        Id,
    ),
}

#[derive(Debug, Clone)]
pub struct DepthInfo {
    /// if we should expand the nested items when serializing
    pub expand: usize,
    /// for recursive generation (ie. if an enum is recursive (Box<Self>))
    pub generate: usize,
    /// for iterative generation
    pub iterate: usize,
}

impl NodeType {
    pub fn iterable_size(&self) -> usize {
        if let Self::Iterable(size, _) = self {
            return *size;
        } else {
            unreachable!("____ADtOHgTK")
        }
    }
    pub fn inner_id(&self) -> Id {
        if let Self::Iterable(_, inner_id) = self {
            return inner_id.clone();
        } else {
            unreachable!("____ADtOHgTK")
        }
    }
}
#[derive(Debug, Clone)]
pub struct Visitor {
    depth: DepthInfo,
    strings: Vec<String>,
    fields: Vec<Vec<((usize, NodeType), Id)>>,
    fields_stack: Vec<((usize, NodeType), Id)>,
    matching_cmps: Vec<(Vec<((usize, NodeType), Id)>, Vec<u8>)>,
    rng: StdRand,
}

pub const ERR_REMAIN_DEPTH: &str = "invariant; we should never be able to go over remaining_depth";
pub const ERR_OVERFLOW: &str = "invariant; you tree is too large? lol";

impl Visitor {
    pub fn get_string(&mut self) -> String {
        let string_count = self.strings.len() - 1;
        let index = self.random_range(0, string_count);
        self.strings.get(index).expect("5hxil4dq____").clone()
    }
    // ADD STRINGS FROM AUTOTOKENS
    pub fn register_string(&mut self, string: String) {
        self.strings.push(string);
    }

    pub fn generate_bytes(&mut self, amount: usize) -> Vec<u8> {
        // TODO: possible to make more efficient?
        (0..amount)
            .map(|_| self.rng.next() as u8)
            .collect::<Vec<_>>()
    }

    pub fn coinflip(&mut self) -> bool {
        self.rng.coinflip(0.5)
    }

    pub fn coinflip_with_prob(&mut self, prob: f64) -> bool {
        // NOTE: depth should always be > 0;
        // we make sure of this cause we don't call this func if not depth > 0
        self.rng.coinflip(prob)
    }

    pub fn random_range(&mut self, min: usize, max: usize) -> usize {
        self.rng.between(min, max)
    }

    pub fn register_field(&mut self, item: ((usize, NodeType), Id)) {
        self.fields_stack.push(item);
        self.fields.push(self.fields_stack.clone());
    }

    pub fn register_cmp(&mut self, data: Vec<u8>) {
        self.matching_cmps.push((self.fields_stack.clone(), data));
    }

    pub fn register_field_stack(&mut self, item: ((usize, NodeType), Id)) {
        self.fields_stack.push(item);
    }

    pub fn expand(&mut self) -> bool {
        self.fields_stack.len() < self.depth.expand
    }

    pub fn get_depth() {}

    pub fn get_rng(&mut self) -> &mut StdRand {
        &mut self.rng
    }

    pub fn generate_depth(&self) -> usize {
        self.depth.generate
    }

    pub fn iterate_depth(&self) -> usize {
        self.depth.iterate
    }

    pub fn pop_field(&mut self) {
        self.fields_stack.pop();
    }

    pub fn cmps(&mut self) -> Vec<(Vec<((usize, NodeType), Id)>, Vec<u8>)> {
        let cmps = self.matching_cmps.clone();
        self.matching_cmps = vec![];
        self.fields = vec![];
        self.fields_stack = vec![];
        cmps
    }

    pub fn fields(&mut self) -> Vec<Vec<((usize, NodeType), Id)>> {
        let fields = self.fields.clone();
        self.fields = vec![];
        self.fields_stack = vec![];
        fields
    }

    pub fn new(seed: u64, depth: DepthInfo) -> Self {
        let mut visitor = Self {
            depth,
            fields: vec![],
            fields_stack: vec![],
            matching_cmps: vec![],
            strings: vec![],
            rng: StdRand::with_seed(seed),
        };
        while visitor.strings.len() < 100 {
            let element_count = visitor.random_range(1, 10);
            let printables =
                "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".as_bytes();
            let res = (0..element_count)
                .map(|_| printables[visitor.random_range(0, printables.len() - 1)])
                .collect::<Vec<u8>>();
            let string = String::from_utf8(res).unwrap();
            if !visitor.strings.contains(&string) {
                visitor.strings.push(string);
            }
        }
        return visitor;
    }
}
