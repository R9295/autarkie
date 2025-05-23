use crate::{FieldLocation, Id, Node, Visitor};
use libafl::{corpus::CorpusId, SerdeAny};
use libafl_bolts::current_time;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
    u128,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputCause {
    Default,
    Generated,
}
#[derive(Debug, Clone, SerdeAny, Serialize, Deserialize)]
pub struct Context {
    mutations: HashSet<MutationMetadata>,
    out_dir: PathBuf,
    type_input_map: HashMap<Id, Vec<PathBuf>>,
    input_cause: InputCause,
}

// TODO: chunk & cmp reloading
impl Context {
    pub fn register_input<I>(&mut self, input: &I, visitor: &mut Visitor)
    where
        I: Node,
    {
        let generated_fields = match &self.input_cause {
            InputCause::Default => visitor.serialized(),
            InputCause::Generated => {
                input.__autarkie_serialized(visitor);
                visitor.serialized()
            }
        };
        for field in generated_fields {
            let (data, ty) = field;
            // todo: optimize this
            let path = self.out_dir.join("chunks").join(ty.to_string());
            match std::fs::create_dir(&path) {
                Ok(_) => {}
                Err(e) => {
                    if !matches!(e.kind(), ErrorKind::AlreadyExists) {
                        panic!("{:?}", e)
                    }
                }
            };
            let hash = blake3::hash(&data);
            let path = path.join(hash.to_string());
            if !std::fs::exists(&path).unwrap() {
                std::fs::write(&path, data).unwrap();
                if let Some(e) = self.type_input_map.get_mut(&ty) {
                    e.push(path);
                } else {
                    self.type_input_map.insert(ty, vec![path]);
                }
            }
        }
        self.input_cause = InputCause::Default;
    }

    pub fn generated_input(&mut self) {
        self.input_cause = InputCause::Generated;
    }

    pub fn default_input(&mut self) {
        self.input_cause = InputCause::Default;
    }

    pub fn add_existing_chunk(&mut self, path: PathBuf) {
        let ty = path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<Id>()
            .expect("corrupt chunk ID!");
        if let Some(e) = self.type_input_map.get_mut(&ty) {
            e.push(path);
        } else {
            self.type_input_map.insert(ty, vec![path]);
        }
    }

    pub fn get_inputs_for_type(&self, t: &Id) -> Option<&Vec<PathBuf>> {
        self.type_input_map.get(t)
    }
}

impl Context {
    pub fn new(out_dir: PathBuf) -> Self {
        let type_input_map = HashMap::default();
        Self {
            mutations: HashSet::new(),
            input_cause: InputCause::Default,
            out_dir,
            type_input_map,
        }
    }

    pub fn add_mutation(&mut self, m: MutationMetadata) {
        self.mutations.insert(m);
    }

    pub fn clear_mutations(&mut self) -> HashSet<MutationMetadata> {
        let cloned = self.mutations.clone();
        self.mutations = HashSet::new();
        cloned
    }
}

/// Track why a testcase was added to the corpus.
#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    SerdeAny,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
)]
pub enum MutationMetadata {
    /// Splice Full Iterable
    SpliceFull,
    /// Splice Single Node (never an iterable)
    SpliceSingle,
    /// Splice Partial Iterable
    SpliceSubSplice,
    /// Splice Append
    SpliceAppend,
    /// Splice Single Node (never an iterable)
    RecurseMutateSingle,
    /// Random Generate Partial Iterable
    RecurseMutateSubsplice,
    /// RecursiveMinimization
    RecursiveMinimization,
    /// Iterable Minimization
    IterableMinimization,
    /// Novelty Minimization
    NoveltyMinimization,
    /// Afl
    Afl,
    /// Generate
    Generate,
    /// Cmplog
    Cmplog,
    /// I2S
    I2S,
}
