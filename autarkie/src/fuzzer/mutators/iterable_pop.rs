use crate::Visitor;
use crate::{MutationType, Node};
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand},
    HasMetadata,
};
use libafl_bolts::{current_time, AsSlice, Named};
use std::collections::HashMap;
use std::path::PathBuf;
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::fuzzer::context::Context;

use super::commons::{calculate_subslice_bounds, FileCache};

pub const SPLICE_STACK: usize = 1000;

pub struct AutarkieIterablePopMutator<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieIterablePopMutator<I>
where
    I: Node,
    S: HasCorpus<I> + HasRand + HasMetadata,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        let mut metadata = state.metadata_mut::<Context>()?;
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();
        if fields.is_empty() {
            return Ok(MutationResult::Skipped);
        }
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &fields[field_splice_index];
        let ((_, node_ty), _) = field.last().expect("EfxPNdQ0____");
        if let crate::NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
            if !is_fixed_len && *field_len > 0 {
                let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                let popped = self.visitor.borrow_mut().random_range(0, field_len - 1);
                #[cfg(feature = "debug_mutators")]
                println!("iterable_pop | one | {:?}", field);
                input.__autarkie_mutate(
                    &mut MutationType::IterablePop(popped),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
                metadata.add_mutation(crate::fuzzer::context::MutationMetadata::IterablePop);
                return Ok(MutationResult::Mutated);
            }
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

impl<I> Named for AutarkieIterablePopMutator<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("AutarkieIterablePopMutator")
    }
}
impl<I> AutarkieIterablePopMutator<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}
