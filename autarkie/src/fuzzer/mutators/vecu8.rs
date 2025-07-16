use crate::Visitor;
use libafl::{mutators::MutatorsTuple, state::HasMaxSize};
use crate::{MutationType, Node};
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand},
    HasMetadata,
};
use libafl_bolts::{rands::Rand, HasLen};
use libafl_bolts::{current_time, AsSlice, Named};
use num_traits::ToBytes;
use std::{collections::HashMap, num::NonZero};
use std::path::PathBuf;
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};
use libafl::mutators::havoc_mutations_no_crossover;
use crate::fuzzer::context::Context;

use super::commons::{calculate_subslice_bounds, FileCache};

pub const SPLICE_STACK: usize = 1000;

pub struct AutarkieVecU8Mutator<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieVecU8Mutator<I>
where
    I: Node,
    S: HasCorpus<I> + HasRand + HasMetadata + HasMaxSize,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &fields[field_splice_index];
        let ((id, node_ty), ty) = field.last().expect("EfxPNdQ0____");
        if let crate::NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
            if *inner_ty == std::intrinsics::type_id::<u8>() {
                let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                let serialized_old = self.visitor.borrow_mut().serialized().clone();
                input.__autarkie_mutate(
                    &mut MutationType::GenerateReplace(420),
                    &mut self.visitor.borrow_mut(),
                    path.clone(),
                );
                let data = self.visitor.borrow_mut().serialized().clone();
                let mut data = data.first().unwrap().0[8..].to_vec();
                let mut mutator = havoc_mutations_no_crossover();
                let mutation = state
                    .rand_mut()
                    .below(unsafe { NonZero::new(mutator.len()).unwrap_unchecked() })
                    .into();
                mutator.get_and_mutate(mutation, state, &mut data);
                let mut length_header = crate::serialize_vec_len(data.len());
                length_header.extend_from_slice(&data);
                input.__autarkie_mutate(
                    &mut MutationType::Splice(&mut length_header.as_slice()),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
                let mut metadata = state.metadata_mut::<Context>()?;
                metadata.add_mutation(crate::fuzzer::context::MutationMetadata::Random);
                metadata.generated_input();
                for item in serialized_old.into_iter() {
                    self.visitor.borrow_mut().add_serialized(item.0, item.1);
                }
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

impl<I> Named for AutarkieVecU8Mutator<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("AutarkieVecU8Mutator")
    }
}
impl<I> AutarkieVecU8Mutator<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>, max_subslice_size: usize) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}
