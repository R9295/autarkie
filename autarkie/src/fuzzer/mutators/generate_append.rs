use crate::fuzzer::context::MutationMetadata;
use crate::Node;
use crate::Visitor;
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand},
    HasMetadata,
};
use libafl_bolts::{AsSlice, Named};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::fuzzer::context::Context;
use super::commons::is_iterable_field;

pub const SPLICE_APPEND_STACK: usize = 1000;
pub struct AutarkieGenerateAppendMutator<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieGenerateAppendMutator<I>
where
    I: Node,
    S: HasCorpus<I> + HasRand + HasMetadata,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        let mut metadata = state.metadata_mut::<Context>().expect("YAERLTe6____");
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self
            .visitor
            .borrow_mut()
            .fields()
            .into_iter()
            .filter(|inner| is_iterable_field(inner))
            .collect::<Vec<_>>();
        if fields.is_empty() {
            return Ok(MutationResult::Skipped);
        }
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &fields[field_splice_index];
        let ((_, node_ty), _) = field.last().expect("jJeuJLG8____");
        if let crate::NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
            if *is_fixed_len {
                return Ok(MutationResult::Skipped);
            }
            let iter_count = self.visitor.borrow().iterate_depth();
            let append_count = self.visitor.borrow_mut().random_range(1, iter_count);
            let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            for _ in 0..append_count {
                let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                let mut bias = if self.visitor.borrow_mut().coinflip() {
                    self.visitor.borrow().generate_depth()
                } else {
                    0
                };
                input.__autarkie_mutate(
                    &mut crate::MutationType::GenerateAppend(bias),
                    &mut self.visitor.borrow_mut(),
                    path.clone(),
                );
            }
            metadata.add_mutation(crate::fuzzer::context::MutationMetadata::GenerateAppend);
            return Ok(MutationResult::Mutated);
        } else {
            return Ok(MutationResult::Skipped);
        }
        Ok(MutationResult::Skipped)
    }

    fn post_exec(
        &mut self,
        _state: &mut S,
        _new_corpus_id: Option<libafl::corpus::CorpusId>,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl<I> Named for AutarkieGenerateAppendMutator<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("AutarkieGenerateAppendMutator")
    }
}
impl<I> AutarkieGenerateAppendMutator<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}
