use crate::Id;
use libafl_bolts::rands::{Rand, StdRand};
use num_traits::CheckedSub;
use petgraph::{
    data::Build,
    dot::{Config, Dot},
    graph::DiGraph,
    graphmap::DiGraphMap,
    Directed,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The `Visitor` struct is the primary way to communicate with the Fuzz-ed type during runtime.
/// Unforuntately procedural macros are rather limiting, so we must delegate effort to the runtime.
/// For example, it is impossible to statically know what fields a struct may have due to Enums.
#[derive(Debug, Clone)]
pub struct Visitor {
    depth: DepthInfo,
    strings: StringPool,

    fields: Vec<Vec<FieldLocation>>,
    field_stack: Vec<FieldLocation>,
    matching_cmps: Vec<(Vec<FieldLocation>, Vec<u8>)>,

    ty_map: BTreeMap<Id, BTreeMap<usize, BTreeSet<Id>>>,
    ty_done: BTreeSet<Id>,
    ty_map_stack: Vec<Id>,

    ty_generate_map: BTreeMap<Id, BTreeMap<GenerateType, BTreeSet<usize>>>,
    rng: StdRand,
}

/// Pool of Strings used by the fuzzer
#[derive(Debug, Clone)]
pub struct StringPool {
    strings: Vec<String>,
}

impl StringPool {
    /// Fetch a random string from the string pool
    pub fn get_string(&mut self, r: &mut StdRand) -> String {
        let string_count = self.strings.len() - 1;
        let index = r.between(0, string_count);
        self.strings.get(index).expect("5hxil4dq____").clone()
    }

    pub fn new() -> Self {
        Self { strings: vec![] }
    }

    /// Add a string manually
    pub fn register_string(&mut self, string: String) {
        if !self.strings.contains(&string) {
            self.strings.push(string);
        }
    }

    /// Add `num` amount of unique strings of `max_len`
    pub fn add_strings(&mut self, r: &mut StdRand,  num: usize, max_len: usize) {
        while self.strings.len() < num {
            let element_count = r.between(1, max_len);
            let printables =
                "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".as_bytes();
            let res = (0..element_count)
                .map(|_| printables[r.between(0, printables.len() - 1)])
                .collect::<Vec<u8>>();
            let string = String::from_utf8(res).unwrap();
            if !self.strings.contains(&string) {
                self.strings.push(string);
            }
        }
    }
}

impl Visitor {
    pub fn get_string(&mut self) -> String {
        self.strings.get_string(&mut self.rng)
    }
    pub fn register_string(&mut self, string: String) {
        self.strings.register_string(string)
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

    pub fn register_field(&mut self, item: FieldLocation) {
        self.field_stack.push(item);
        self.fields.push(self.field_stack.clone());
    }

    pub fn register_cmp(&mut self, data: Vec<u8>) {
        self.matching_cmps.push((self.field_stack.clone(), data));
    }

    pub fn register_field_stack(&mut self, item: FieldLocation) {
        self.field_stack.push(item);
    }

    pub fn pop_field(&mut self) {
        self.field_stack.pop();
    }

    pub fn cmps(&mut self) -> Vec<(Vec<FieldLocation>, Vec<u8>)> {
        let cmps = std::mem::take(&mut self.matching_cmps);
        self.fields.clear();
        self.field_stack.clear();
        cmps
    }

    pub fn fields(&mut self) -> Vec<Vec<FieldLocation>> {
        let fields = std::mem::take(&mut self.fields);
        self.field_stack.clear();
        fields
    }

    pub fn generate_depth(&self) -> usize {
        self.depth.generate
    }

    pub fn iterate_depth(&self) -> usize {
        self.depth.iterate
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
        let popped = self.ty_map_stack.pop().expect("____mZiIy3hlu8");
        self.ty_done.insert(popped);
    }

    pub fn is_recursive(&mut self, id: Id) -> bool {
        self.ty_map_stack.contains(&id) || self.ty_done.contains(&id)
    }

    // TODO: optimize
    // TODO: refactor ffs
    pub fn calculate_recursion(&mut self) -> BTreeMap<Id, BTreeSet<usize>> {
        let mut recursive_nodes = BTreeMap::new();
        let mut g = DiGraphMap::<_, usize>::new();
        for (ty, variants) in self.ty_map.iter() {
            for (variant_id, variant_tys) in variants {
                g.add_edge((ty, -1), (ty, *variant_id as isize), 1);
                for variant_ty in variant_tys {
                    g.add_edge((ty, *variant_id as isize), (variant_ty, -1), 1);
                }
            }
        }
        let cycles = crate::graph::find_cycles(&g);
        for cycle in cycles {
            let (root_ty, root_variant) = cycle.first().unwrap();
            let root = self.ty_map.get(cycle.first().unwrap().0).unwrap();
            let (last_ty, last_variant) = cycle.last().unwrap();
            let last = self.ty_map.get(cycle.last().unwrap().0).unwrap();
            if *root_ty == *last_ty {
                // a type may be recursive to it's own so we ignore
                if last_variant.gt(&-1) {
                    recursive_nodes
                        .entry(root_ty.clone().clone())
                        .and_modify(|inner: &mut BTreeSet<usize>| {
                            inner.insert(last_variant.clone().try_into().unwrap_or(0));
                        })
                        .or_insert(BTreeSet::from_iter([last_variant
                            .clone()
                            .try_into()
                            .unwrap_or(0)]));
                }
            } else {
                let root_index = 1;
                let last_index = cycle.len().checked_sub(1).unwrap();
                let (root_ty, root_variant) = cycle.get(root_index).unwrap();
                let (last_ty, last_variant) = cycle.get(last_index).unwrap();
                let root_variant_count = self.ty_map.get(root_ty.clone()).unwrap().len();
                let last_variant_count = self.ty_map.get(last_ty.clone()).unwrap().len();
                if root_variant_count > last_variant_count {
                    recursive_nodes
                        .entry(root_ty.clone().clone())
                        .and_modify(|inner: &mut BTreeSet<usize>| {
                            inner.insert(root_variant.clone().try_into().unwrap_or(0));
                        })
                        .or_insert(BTreeSet::from_iter([root_variant
                            .clone()
                            .try_into()
                            .unwrap_or(0)]));
                } else if last_variant_count > root_variant_count {
                    recursive_nodes
                        .entry(last_ty.clone().clone())
                        .and_modify(|inner: &mut BTreeSet<usize>| {
                            inner.insert(last_variant.clone().try_into().unwrap_or(0));
                        })
                        .or_insert(BTreeSet::from_iter([last_variant
                            .clone()
                            .try_into()
                            .unwrap_or(0)]));
                }
            }
        }
        for (ty, map) in &self.ty_map {
            let recursive_variants = recursive_nodes.get(ty);
            if let Some(recursive_variants) = recursive_variants {
                let r_variants = recursive_variants.clone();
                self.ty_generate_map.insert(
                    ty.clone(),
                    BTreeMap::from_iter([(GenerateType::Recursive, r_variants)]),
                );
            }
            let mut nr_variants = map.keys().cloned().collect::<BTreeSet<_>>();
            if let Some(recursive_variants) = recursive_variants {
                nr_variants = nr_variants
                    .into_iter()
                    .filter(|item| !recursive_variants.contains(item))
                    .collect::<BTreeSet<_>>();
            }
            self.ty_generate_map
                .entry(ty.clone())
                .and_modify(|inner| {
                    inner.insert(GenerateType::NonRecursive, nr_variants.clone());
                })
                .or_insert(BTreeMap::from_iter([(
                    GenerateType::NonRecursive,
                    nr_variants,
                )]));
        }
        return recursive_nodes;
    }
    #[inline]
    /// This function is used by enums to determine which variant to generate.
    /// Since some variant are recursive, we check whether our depth is under the recursive depth
    /// limit.
    pub fn generate(&mut self, id: &Id, depth: &usize) -> usize {
        let consider_recursive = *depth < self.depth.generate;
        let variant = if consider_recursive {
            let variants = self.ty_generate_map.get(id).expect("____VbO3rGYTSf");
            let nr_variants = variants
                .get(&GenerateType::NonRecursive)
                .expect("____lCAftArdHS");
            let r_variants = variants
                .get(&GenerateType::Recursive)
                .expect("____q154Wl5zf2");
            let id = self.rng.between(0, r_variants.len() + nr_variants.len());
            if id < nr_variants.len() {
                nr_variants.get(&id).expect("____ql6E70MLIb").clone()
            } else {
                r_variants.get(&id.checked_sub(nr_variants.len()).expect("____ibvCjQB5oX")).expect("____LaawYczeqc").clone()
            }
        } else {
            let variants = self
                .ty_generate_map
                .get(id)
                .expect("____clESlzqUbX")
                .get(&GenerateType::NonRecursive)
                .expect("____ffxyyA6Nub");
            let id = self.rng.between(0, variants.len());
            variants.get(&id).expect("____pvPK973BLH").clone()
        };
        variant
    }

    pub fn new(seed: u64, depth: DepthInfo) -> Self {
        let mut visitor = Self {
            ty_generate_map: BTreeMap::default(),
            ty_done: BTreeSet::default(),
            ty_map_stack: vec![],
            depth,
            fields: vec![],
            field_stack: vec![],
            matching_cmps: vec![],
            strings: StringPool::new(),
            ty_map: BTreeMap::new(),
            rng: StdRand::with_seed(seed),
        };
        visitor.strings.add_strings(&mut visitor.rng, 100, 10);
        return visitor;
    }
}

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

impl NodeType {
    pub fn is_recursive(&self) -> bool {
        matches!(self, NodeType::Recursive)
    }
    
    pub fn is_iterable(&self) -> bool {
        matches!(self, NodeType::Iterable(_, _, _))
    }
}

#[derive(Debug, Clone)]
pub struct DepthInfo {
    /// for recursive generation (eg. if an enum is recursive (Box<Self>))
    pub generate: usize,
    /// for iterative generation (Vec/HashMap)
    pub iterate: usize,
}

#[derive(Ord, PartialEq, Eq, PartialOrd, Debug, Clone)]
enum GenerateType {
    Recursive,
    NonRecursive,
}

pub type FieldLocation = ((usize, NodeType), Id);
