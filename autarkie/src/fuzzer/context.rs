use crate::{Id, Node};
use libafl::{corpus::CorpusId, SerdeAny};
use libafl_bolts::current_time;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
    u128,
};

#[derive(Debug, Clone, SerdeAny, Serialize, Deserialize)]
pub struct Context {
    out_dir: PathBuf,
    type_input_map: HashMap<Id, Vec<PathBuf>>,
}

// TODO: chunk & cmp reloading
impl Context {
    pub fn register_input<I>(&mut self, input: &I)
    where
        I: Node,
    {
        for field in input.__autarkie_serialized().unwrap() {
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
            out_dir,
            type_input_map,
        }
    }
}
