use crate::Visitor;
use crate::{MutationType, Node};
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

pub struct AutarkieRandomMutator<I> {
    max_subslice_size: usize,
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieRandomMutator<I>
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
        let field = &mut fields[field_splice_index];
        let ((_, node_ty), _) = field.last().expect("YjBYG4Fr____");
        let bias = self.visitor.borrow().generate_depth();
        if let crate::NodeType::Iterable(_, field_len, inner_ty) = node_ty {
            if *field_len < 3 {
                return Ok(MutationResult::Skipped);
            }
            let subslice_bounds = calculate_subslice_bounds(
                *field_len,
                self.max_subslice_size,
                &mut self.visitor.borrow_mut(),
            );
            for index in subslice_bounds {
                let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                path.push_back(index);
                #[cfg(feature = "debug_mutators")]
                println!("recursive_mutate | subslice | {:?}", field);
                input.__autarkie_mutate(
                    &mut MutationType::GenerateReplace(bias / 2),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
            }
            metadata.add_mutation(crate::fuzzer::context::MutationMetadata::RandomMutateSubsplice);
        } else {
            let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            #[cfg(feature = "debug_mutators")]
            println!("recursive_mutate | single | {:?}", field);
            input.__autarkie_mutate(
                &mut MutationType::GenerateReplace(bias),
                &mut self.visitor.borrow_mut(),
                path,
            );
            metadata.add_mutation(crate::fuzzer::context::MutationMetadata::RandomMutateSingle);
        }
        Ok(MutationResult::Mutated)
    }

    fn post_exec(
        &mut self,
        _state: &mut S,
        _new_corpus_id: Option<libafl::corpus::CorpusId>,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl<I> Named for AutarkieRandomMutator<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("AutarkieRandomMutator")
    }
}
impl<I> AutarkieRandomMutator<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>, max_subslice_size: usize) -> Self {
        Self {
            visitor,
            max_subslice_size,
            phantom: PhantomData,
        }
    }
}
