use crate::Node;
use crate::Visitor;
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand},
    HasMetadata,
};
#[cfg(feature = "introspection")]
use libafl::{mark_feature_time, start_timer};
use libafl_bolts::{AsSlice, Named};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::fuzzer::Context;

pub struct AutarkieSpliceAppendMutator<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieSpliceAppendMutator<I>
where
    I: Node,
    S: HasCorpus<I> + HasRand + HasMetadata,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        let mut metadata = state.metadata_mut::<Context>().unwrap();
        #[cfg(feature = "introspection")]
        start_timer!(state);
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        #[cfg(feature = "introspection")]
        mark_feature_time!(state, Data::Fields);
        let mut fields = self
            .visitor
            .borrow_mut()
            .fields()
            .into_iter()
            .filter(|inner| {
                let last = inner.last().as_ref().unwrap();
                matches!(&crate::NodeType::Iterable, last)
            })
            .collect::<Vec<_>>();
        if fields.len() == 0 {
            return Ok(MutationResult::Skipped);
        }
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &fields[field_splice_index];
        let ((id, node_ty), ty) = field.last().unwrap();
        if let crate::NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
            if *is_fixed_len {
                return Ok(MutationResult::Skipped);
            }
            if let Some(possible_splices) = metadata.get_inputs_for_type(&inner_ty) {
                if *field_len > 200 {
                    return Ok(MutationResult::Skipped);
                }
                // calculate subsplice size
                let iter_count = self.visitor.borrow().iterate_depth();
                let append_count = self.visitor.borrow_mut().random_range(1, iter_count);
                let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                for _ in 0..append_count {
                    let random_splice = possible_splices
                        .get(
                            self.visitor
                                .borrow_mut()
                                .random_range(0, possible_splices.len() - 1),
                        )
                        .unwrap();
                    // TODO: cache this in memory
                    let data = std::fs::read(random_splice).unwrap();
                    #[cfg(debug_assertions)]
                    println!("splice | splice_append | {:?}", (&field, &path));
                    input.__autarkie_mutate(
                        &mut crate::MutationType::SpliceAppend(&mut data.as_slice()),
                        &mut self.visitor.borrow_mut(),
                        path.clone(),
                    );
                }
                return Ok(MutationResult::Mutated);
            } else {
                return Ok(MutationResult::Skipped);
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

impl<I> Named for AutarkieSpliceAppendMutator<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("AutarkieSpliceAppendMutator")
    }
}
impl<I> AutarkieSpliceAppendMutator<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}
