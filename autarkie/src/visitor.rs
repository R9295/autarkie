use std::collections::{BTreeMap, BTreeSet, HashMap};

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
        /// if fixed element count (array)
        bool,
        /// Amount of Elements
        usize,
        /// type id of Elements
        Id,
    ),
}

#[derive(Debug, Clone)]
pub struct DepthInfo {
    /// for recursive generation (ie. if an enum is recursive (Box<Self>))
    pub generate: usize,
    /// for iterative generation
    pub iterate: usize,
}

impl NodeType {
    pub fn iterable_size(&self) -> usize {
        if let Self::Iterable(_, size, _) = self {
            return *size;
        } else {
            unreachable!("____ADtOHgTK")
        }
    }
    pub fn inner_id(&self) -> Id {
        if let Self::Iterable(_, _, inner_id) = self {
            return inner_id.clone();
        } else {
            unreachable!("____ADtOHgTK")
        }
    }
    pub fn is_fixed(&self) -> bool {
        if let Self::Iterable(is_fixed, _, _) = self {
            return is_fixed.clone();
        } else {
            unreachable!("____ADtOHgTK")
        }
    }
}

#[derive(Debug, Clone)]
pub enum InnerNodeType {
    Recursive,
    NonRecursive,
}
#[derive(Debug, Clone)]
pub struct Visitor {
    depth: DepthInfo,
    strings: Vec<String>,
    fields: Vec<Vec<((usize, NodeType), Id)>>,
    fields_stack: Vec<((usize, NodeType), Id)>,
    matching_cmps: Vec<(Vec<((usize, NodeType), Id)>, Vec<u8>)>,
    ty_map: BTreeMap<Id, BTreeMap<usize, BTreeSet<Id>>>,
    recursive_nodes: BTreeMap<Id, BTreeMap<InnerNodeType, Vec<usize>>>,
    ty_map_stack: Vec<Id>,
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

    pub fn register_ty(&mut self, parent: Option<Id>, id: Id, variant: usize) {
        self.ty_map_stack.push(id.clone());
        let parent = parent.unwrap_or("AutarkieInternalFuzzData".to_string());
        if !self.ty_map.get(&parent).is_some() {
            self.ty_map.insert(
                parent.clone(),
                BTreeMap::from_iter([(variant, BTreeSet::new())]),
            );
        }
        self.ty_map
            .get_mut(&parent)
            .expect("____rwBG5LkVKH")
            .entry(variant)
            .and_modify(|i| {
                i.insert(id.clone());
            })
            .or_insert(BTreeSet::from_iter([id.clone()]));
    }

    pub fn pop_ty(&mut self) {
        self.ty_map_stack.pop().expect("____mZiIy3hlu8");
    }

    pub fn is_recursive(&mut self, id: Id) -> bool {
        self.ty_map_stack.contains(&id)
    }

    pub fn calculate_recursion(&mut self) -> BTreeMap<Id, BTreeSet<usize>> {
        use colored::Colorize;
        let mut recursive_nodes = BTreeMap::new();
        let mut reverse_map = BTreeMap::new();
        // take a type, and find everywhere where it's referenced.
        for (ty, variants) in self.ty_map.iter() {
            println!("{:?} has {:?} variants", ty, variants.len());
            let mut who_references_us = vec![];
            // do we reference them
            for (inner_ty, inner_variants) in self.ty_map.iter() {
                for inner_variant in inner_variants {
                    if inner_variant.1.contains(ty) {
                        print!("{}", "----> ".green().bold());
                        println!("variant {:?} of {:?} references us", inner_variant.0, inner_ty);
                        who_references_us.push((inner_variant.0, inner_ty));
                        reverse_map.entry(ty).and_modify(|i: &mut Vec<_>| {i.push((inner_ty.clone(), inner_variant.clone()))}).or_insert(vec![(inner_ty.clone(), inner_variant.clone())]);
                    }
                }
            }
            // do we directly reference anyone who references us?
            for (variant, values) in variants.iter() {
                for (reference_variant, reference_ty) in &who_references_us {
                    if values.contains(reference_ty.clone()) {
                        // find out who has more variants.
                        // whoever has more is the recursive one.
                        let our_varaints = variants.keys().len();
                        let reference_variants =
                            self.ty_map.get(reference_ty.clone()).unwrap().len();
                        if our_varaints < reference_variants
                            || (our_varaints == reference_variants && *reference_ty == ty)
                        {
                            recursive_nodes
                                .entry(reference_ty.clone().clone())
                                .and_modify(|inner: &mut BTreeSet<usize>| {
                                    inner.insert(reference_variant.clone().clone());
                                })
                                .or_insert(BTreeSet::from_iter([reference_variant
                                    .clone()
                                    .clone()]));
                        }
                    }
                }
                // do we indirectly reference anyone who references us?
                for item in &who_references_us {
                   let mut path = vec![];
                   let current = vec![];
                }
            }
        }
        println!("{:#?}", reverse_map);
        return recursive_nodes;
        // if it has alternatives set the variant as recursive, else if we have alternatives, set
        // us as recursive, else, panic
    }

    pub fn print_ty(&self) {
        println!("{:#?}", self.ty_map);
        /* println!("recursive");
        println!("{:#?}", self.recursive_nodes); */
    }
    pub fn new(seed: u64, depth: DepthInfo) -> Self {
        let mut visitor = Self {
            recursive_nodes: BTreeMap::new(),
            ty_map_stack: vec![],
            depth,
            fields: vec![],
            fields_stack: vec![],
            matching_cmps: vec![],
            strings: vec![],
            ty_map: BTreeMap::new(),
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
