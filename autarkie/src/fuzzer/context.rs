use crate::{FieldLocation, Id, Node, Visitor};
use libafl::{corpus::CorpusId, inputs::ToTargetBytes, SerdeAny};
use libafl_bolts::current_time;
use libafl_bolts::AsSlice;
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
    render: bool,
    mutations: HashSet<MutationMetadata>,
    out_dir: PathBuf,
    pub type_input_map: HashMap<Id, Vec<PathBuf>>,
    input_cause: InputCause,
}

// TODO: chunk & cmp reloading
impl Context {
    pub fn register_input<I, TC>(
        &mut self,
        input: &I,
        visitor: &mut Visitor,
        converter: &mut TC,
        is_solution: bool,
    ) where
        TC: ToTargetBytes<I>,
        I: Node,
    {
        self.store_generated_chunks(input, visitor);
        let rendered = converter.to_target_bytes(&input);
        let render_dir = if is_solution {
            self.out_dir.join("rendered_crashes")
        } else {
            self.out_dir.join("rendered_corpus")
        };
        ensure_dir(&render_dir);
        let render_path = render_dir.join(twox_hash::XxHash64::oneshot(0, &rendered).to_string());
        if !std::fs::exists(&render_path).unwrap() {
            // warn that the same input gave new coverage == instability!
            std::fs::write(&render_path, rendered.as_slice()).unwrap();
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
    pub fn new(out_dir: PathBuf, render: bool) -> Self {
        let type_input_map = HashMap::default();
        Self {
            mutations: HashSet::new(),
            input_cause: InputCause::Default,
            out_dir,
            type_input_map,
            render,
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

fn ensure_dir(path: &Path) {
    if let Err(e) = std::fs::create_dir(path) {
        if !matches!(e.kind(), ErrorKind::AlreadyExists) {
            panic!("{:?}", e);
        }
    }
}

impl Context {
    fn store_generated_chunks<I>(&mut self, input: &I, visitor: &mut Visitor)
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
        let string_ty = String::__autarkie_id();
        for (mut data, ty) in generated_fields {
            if ty == string_ty {
                visitor.register_string(crate::deserialize(&mut data.as_slice()));
            }
            self.store_chunk_for_type(ty, &data);
        }
    }

    fn store_chunk_for_type(&mut self, ty: Id, data: &[u8]) {
        let chunk_dir = self.out_dir.join("chunks").join(ty.to_string());
        ensure_dir(&chunk_dir);
        let chunk_path = chunk_dir.join(twox_hash::XxHash64::oneshot(0, data).to_string());
        if std::fs::exists(&chunk_path).unwrap() {
            return;
        }
        std::fs::write(&chunk_path, data).unwrap();
        self.type_input_map
            .entry(ty)
            .and_modify(|entries| entries.push(chunk_path.clone()))
            .or_insert_with(|| vec![chunk_path]);
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
    /// Generate Append
    GenerateAppend,
    /// Splice Single Node (never an iterable)
    RandomMutateSingle,
    /// Random Generate Partial Iterable
    RandomMutateSubsplice,
    /// RecursiveMinimization
    RecursiveMinimization,
    /// Iterable Minimization
    IterableMinimization,
    /// Iterable Pop
    IterablePop,
    /// Novelty Minimization
    NoveltyMinimization,
    /// Afl
    Afl,
    /// Generate
    Generate,
    /// Cmplog
    Cmplog,
    /// CmplogBytes
    CmplogBytes,
    /// I2S
    I2S,
    Random,
}
