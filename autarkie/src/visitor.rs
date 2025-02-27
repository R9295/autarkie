use std::collections::{BTreeMap, BTreeSet, HashMap};

use libafl_bolts::{
    rands::{Rand, StdRand},
    HasLen,
};
use petgraph::{
    data::Build,
    dot::{Config, Dot},
    graph::DiGraph,
    graphmap::DiGraphMap,
    Directed,
};

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

#[derive(Ord, PartialEq, Eq, PartialOrd, Debug, Clone)]
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
    ty_done: BTreeSet<Id>,
    ty_map_stack: Vec<Id>,

    ty_generate_map: BTreeMap<Id, BTreeMap<InnerNodeType, BTreeSet<usize>>>,
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
                    BTreeMap::from_iter([(InnerNodeType::Recursive, r_variants)]),
                );
            }
            let mut nr_variants = map.keys().cloned().collect::<BTreeSet<_>>();
            if let Some(recursive_variants) = recursive_variants {
                nr_variants = nr_variants
                    .into_iter()
                    .filter(|item| !recursive_variants.contains(item))
                    .collect::<BTreeSet<_>>();
            }
            self.ty_generate_map.entry(ty.clone()).and_modify(|inner| {inner.insert(InnerNodeType::NonRecursive, nr_variants.clone());}).or_insert(BTreeMap::from_iter([(InnerNodeType::NonRecursive, nr_variants)]));
        }
        return recursive_nodes;
    }

    #[inline]
    pub fn generate(&mut self, id: &Id, depth: &usize) -> usize {
       let consider_recursive = *depth < self.depth.generate;
       let variant = if consider_recursive {
            let variants = self.ty_generate_map.get(id).expect("____VbO3rGYTSf");
            let nr_variants = variants.get(&InnerNodeType::NonRecursive).expect("____lCAftArdHS");
            let r_variants = variants.get(&InnerNodeType::Recursive).expect("____q154Wl5zf2");
            let id = self.rng.between(0, nr_variants.len() + r_variants.len());
            let all = nr_variants.iter().chain(r_variants).collect::<Vec<&usize>>();
            all.get(id).expect("____VPPeXUSTFO").clone().clone()
        } else {
            let variants = self.ty_generate_map.get(id).expect("____clESlzqUbX").get(&InnerNodeType::NonRecursive).expect("____ffxyyA6Nub");
            let id = self.rng.between(0, variants.len());
            variants.get(&id).expect("____pvPK973BLH").clone()
        };
        variant
    }

    pub fn print_ty(&self) {
        println!("{:#?}", self.ty_map);
    }
    pub fn new(seed: u64, depth: DepthInfo) -> Self {
        let mut visitor = Self {
            ty_generate_map: BTreeMap::default(),
            ty_done: BTreeSet::default(),
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
