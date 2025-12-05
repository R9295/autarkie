use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
    path::PathBuf,
};

use crate::{FieldLocation, Visitor};

pub fn calculate_subslice_bounds(len: usize, max: usize, visitor: &mut Visitor) -> Range<usize> {
    // minus 1 because we zero index and len is always +1
    let start = visitor.random_range(0, len - 1);
    let mut end = visitor.random_range(start, len);
    if end - start > max {
        end = start + max;
    }
    start..end
}

pub struct FileCache {
    data: HashMap<PathBuf, Vec<u8>>,
    queue: VecDeque<PathBuf>,
    size: usize,
    max_size: usize,
}

impl FileCache {
    pub fn read_cached(&mut self, path: &PathBuf) -> Result<&[u8], std::io::Error> {
        if !self.data.contains_key(path) {
            let data = std::fs::read(path)?;
            self.size += data.len();
            self.data.insert(path.clone(), data);
            self.queue.push_back(path.to_path_buf());
        }
        while self.size > self.max_size {
            let v = self
                .data
                .remove(&self.queue.pop_front().expect("invariant;2"))
                .expect("invariant;");
            self.size -= v.len();
        }
        Ok(self.data.get(path).as_ref().unwrap())
    }

    pub fn new(size: usize) -> Self {
        Self {
            queue: VecDeque::default(),
            max_size: size * 1024 * 1024,
            data: HashMap::new(),
            size: 0,
        }
    }
}

pub fn is_iterable_field(field: &[FieldLocation]) -> bool {
    matches!(
        field.last().map(|location| &location.0 .1),
        Some(crate::NodeType::Iterable(..))
    )
}
