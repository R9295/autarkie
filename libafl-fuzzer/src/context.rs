use libafl::{corpus::CorpusId, SerdeAny};
use libafl_bolts::current_time;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    path::PathBuf,
    u128, time::Duration,
};
use thesis::{Node, Id};

// Note: if we have no enums, then this is redundant cause everyone will have the same fields all
// the time
#[derive(Debug, Clone, SerdeAny, Serialize, Deserialize)]
pub struct Context {
    out_dir: PathBuf,

    // types mapped to their inputs
    type_input_map: HashMap<Id, Vec<PathBuf>>,

    // path of nodes known to be observed in comparisons
    interesting_nodes: HashSet<Vec<usize>>,
    cmp_input_map: HashMap<u128, Vec<PathBuf>>,
}
// TODO: chunk & cmp reloading
impl Context {
    pub fn register_input<I>(&mut self, input: &I, corpus_id: CorpusId)
    where
        I: Node,
    {
/*         let start = current_time(); */
        for field in input.serialized().unwrap() {
            let (data, ty) = field;
            // todo: optimize this
            let path = self.out_dir.join("chunks").join(ty.to_string());
            match std::fs::create_dir(&path) {
                Ok(_) => {}
                Err(e) => {
                    if matches!(e.kind(), ErrorKind::AlreadyExists) {
                    } else {
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
/*         println!("ELAPSED={:?}", (current_time() - start).as_secs_f32()) */
    }

    pub fn get_inputs_for_type(&self, t: &Id) -> Option<&Vec<PathBuf>> {
        self.type_input_map.get(t)
    }
    pub fn interesting_nodes(&self) -> &HashSet<Vec<usize>> {
        &self.interesting_nodes
    }

    pub fn get_cmps_for_path(&self, t: &u128) -> Option<&Vec<PathBuf>> {
        self.cmp_input_map.get(t)
    }

    pub fn add_cmps(&mut self, data: Vec<(Vec<((usize, isize), Id)>, Vec<u8>)>) {
        for (item, serialized) in &data {
            let ty = &item.last().expect("____Ed7Pt03DcM").1;
            let path = self.out_dir.join("cmps").join(ty.to_string());
            let node_path = item
                .iter()
                .map(|((index, size), ty)| *index)
                .collect::<Vec<_>>();
            self.interesting_nodes.insert(node_path);
            match std::fs::create_dir(&path) {
                Ok(_) => {}
                Err(e) => {
                    if matches!(e.kind(), ErrorKind::AlreadyExists) {
                    } else {
                        panic!("{:?}", e)
                    }
                }
            };

            let hash = blake3::hash(serialized);
            let path = path.join(hash.to_string());
            if !std::fs::exists(&path).unwrap() {
                std::fs::write(&path, serialized).unwrap();
                if let Some(e) = self.type_input_map.get_mut(ty) {
                    e.push(path);
                } else {
                    self.type_input_map.insert(ty.clone(), vec![path]);
                }
            }
        }
    }
}

impl Context {
    pub fn new(out_dir: PathBuf) -> Self {
        let type_input_map = HashMap::default();
        let interesting_nodes = HashSet::default();
        let cmp_input_map = HashMap::default();
        Self {
            cmp_input_map,
            out_dir,
            type_input_map,
            interesting_nodes,
        }
    }
}
