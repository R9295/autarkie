use crate::Visitor;
use crate::{MutationType, Node};
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand},
    HasMetadata,
};
#[cfg(feature = "introspection")]
use libafl::{mark_feature_time, start_timer};
use libafl_bolts::{HasLen, Named};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::fuzzer::Context;

use super::commons::calculate_subslice_bounds;

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
        let mut metadata = state.metadata_mut::<Context>()?;
        #[cfg(feature = "introspection")]
        start_timer!(state);
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();
        #[cfg(feature = "introspection")]
        mark_feature_time!(state, Data::Fields);
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &mut fields[field_splice_index];
        let ((id, node_ty), ty) = field.last().unwrap();
        let mut bias = if self.visitor.borrow_mut().coinflip() {
            self.visitor.borrow().generate_depth()
        } else {
            0
        };
        if let crate::NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
            if *field_len < 3 {
                return Ok(MutationResult::Skipped);
            }
            let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            let subslice_bounds = calculate_subslice_bounds(
                *field_len,
                self.max_subslice_size,
                &mut self.visitor.borrow_mut(),
            );
            for index in subslice_bounds {
                let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                path.push_back(index);
                #[cfg(debug_assertions)]
                println!("recursive_mutate | subslice | {:?}", field);
                input.__autarkie_mutate(
                    &mut MutationType::GenerateReplace(bias),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
            }
        } else {
            let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            #[cfg(debug_assertions)]
            println!("recursive_mutate | single | {:?}", field);
            input.__autarkie_mutate(
                &mut MutationType::GenerateReplace(bias),
                &mut self.visitor.borrow_mut(),
                path,
            );
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
