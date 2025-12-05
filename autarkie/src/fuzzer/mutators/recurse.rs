use crate::fuzzer::stages::minimization::shuffle;
use crate::{MutationType, Node};
use crate::{NodeType, Visitor};
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand},
    HasMetadata,
};
use libafl_bolts::{HasLen, Named};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::fuzzer::context::Context;

use super::commons::calculate_subslice_bounds;

pub const RECURSE_STACK: usize = 1000;

pub struct AutarkieRecurseMutator<I> {
    max_subslice_size: usize,
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieRecurseMutator<I>
where
    I: Node,
    S: HasCorpus<I> + HasRand + HasMetadata,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        if !self.visitor.borrow().has_recursive_types() {
            return Ok(MutationResult::Skipped);
        }
        let mut metadata = state.metadata_mut::<Context>()?;
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        // shuffle the fields
        let mut fields = self.visitor.borrow_mut().fields();
        shuffle(&mut fields, &mut self.visitor.borrow_mut());
        // find the first recursive node;
        let mut recursive_node = None;
        for node in &fields {
            if matches!(node.last().expect("____LqG7SD18").0 .1, NodeType::Recursive) {
                recursive_node = Some(node);
            }
        }
        let mut all = Vec::new();
        let mut start = Some(0);
        if let Some(recursive_node) = recursive_node {
            for node in &fields {
                if let Some(index) = find_subsequence(&node, &recursive_node, start) {
                    if fields.get(index).unwrap().len() == recursive_node.len() + 1 {
                        all.push(fields.get(index).unwrap());
                        start = Some(index)
                    }
                }
            }
        }
        return Ok(MutationResult::Skipped);
    }

    fn post_exec(
        &mut self,
        _state: &mut S,
        _new_corpus_id: Option<libafl::corpus::CorpusId>,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl<I> Named for AutarkieRecurseMutator<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("AutarkieRecurseMutator")
    }
}
impl<I> AutarkieRecurseMutator<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>, max_subslice_size: usize) -> Self {
        Self {
            visitor,
            max_subslice_size,
            phantom: PhantomData,
        }
    }
}

fn find_subsequence<T: PartialEq>(
    haystack: &[T],
    needle: &[T],
    start: Option<usize>,
) -> Option<usize> {
    haystack[start.unwrap_or(0)..]
        .windows(needle.len())
        .position(|window| window == needle)
}
