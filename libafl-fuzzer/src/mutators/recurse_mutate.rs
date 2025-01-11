use autarkie::Visitor;
use autarkie::{MutationType, Node};
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand, State},
    HasMetadata,
};
use libafl_bolts::{HasLen, Named};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::context::Context;

pub struct AutarkieRecurseMutator<I> {
    max_subslice_size: usize,
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieRecurseMutator<I>
where
    I: Node,
    S: State + HasCorpus + HasRand + HasMetadata,
    S::Corpus: Corpus<Input = I>,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        input.fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &mut fields[field_splice_index];
        let ((id, node_ty), ty) = field.last().unwrap();
        let mut bias = if self.visitor.borrow_mut().coinflip() {
            self.visitor.borrow().generate_depth()
        } else {
            0
        };
        if matches!(node_ty, autarkie::NodeType::Iterable(_, _)) {
            let field_len = field.last().unwrap().0 .1.iterable_size();
            if field_len < 3 {
                return Ok(MutationResult::Skipped);
            }
            let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            let subslice_start = self.visitor.borrow_mut().random_range(0, field_len - 1);
            let mut subslice_end = self
                .visitor
                .borrow_mut()
                .random_range(subslice_start, field_len);
            if subslice_end - subslice_start > self.max_subslice_size {
                subslice_end = subslice_start + self.max_subslice_size;
            }
            for index in subslice_start..subslice_end {
                let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                path.push_back(index);
                #[cfg(debug_assertions)]
                println!("recursive_mutate | subslice | {:?}", field);
                input.__mutate(
                    &mut MutationType::GenerateReplace(bias),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
            }
        } else {
            let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            #[cfg(debug_assertions)]
            println!("recursive_mutate | single | {:?}", field);
            input.__mutate(
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
