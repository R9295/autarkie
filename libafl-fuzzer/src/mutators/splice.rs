use autarkie::Visitor;
use autarkie::{MutationType, Node};
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand, State},
    HasMetadata,
};
use libafl_bolts::{current_time, AsSlice, Named};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::context::Context;

pub struct AutarkieSpliceMutator<I> {
    visitor: Rc<RefCell<Visitor>>,
    max_subslice_size: usize,
    phantom: PhantomData<I>,
}

impl<I, S> Mutator<I, S> for AutarkieSpliceMutator<I>
where
    I: Node,
    S: State + HasCorpus + HasRand + HasMetadata,
    S::Corpus: Corpus<Input = I>,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        let metadata = state.metadata::<Context>()?;
        input.fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &fields[field_splice_index];
        let ((id, node_ty), ty) = field.last().unwrap();
        if matches!(node_ty, autarkie::NodeType::Iterable(_, _)) {
            let inner_ty = node_ty.inner_id();
            let subslice = self.visitor.borrow_mut().coinflip_with_prob(0.6);
            if subslice {
                // no point subslicing when we have less than 5 entries
                let field_len = field.last().unwrap().0 .1.iterable_size();
                if field_len < 3 {
                    return Ok(MutationResult::Skipped);
                }
                if let Some(possible_splices) = metadata.get_inputs_for_type(&inner_ty) {
                    let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                    let subslice_start = self.visitor.borrow_mut().random_range(0, field_len - 1);
                    let mut subslice_end = self
                        .visitor
                        .borrow_mut()
                        .random_range(subslice_start, field_len);
                    if subslice_end - subslice_start > self.max_subslice_size {
                        subslice_end = subslice_start + self.max_subslice_size;
                    }
                    // calculate subsplice size
                    let subslice_end = field_len;
                    let subslice_start = self.visitor.borrow_mut().random_range(0, field_len - 1);
                    for index in subslice_start..subslice_end {
                        let mut child_path = path.clone();
                        child_path.push_back(index);
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
                        println!("splice | subslice | {:?}", (&field, &path));
                        input.__mutate(
                            &mut MutationType::Splice(&mut data.as_slice()),
                            &mut self.visitor.borrow_mut(),
                            child_path,
                        );
                    }
                } else {
                    return Ok(MutationResult::Skipped);
                }
            } else {
                if let Some(possible_splices) = metadata.get_inputs_for_type(&inner_ty) {
                    // unfortunately we need to replace the exact amount.
                    // cause we don't differentiate between vec and slice
                    let iter_size = node_ty.iterable_size();
                    let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                    let items = (0..iter_size)
                        .into_iter()
                        .map(|_| {
                            std::fs::read(
                                possible_splices
                                    .get(
                                        self.visitor
                                            .borrow_mut()
                                            .random_range(0, possible_splices.len() - 1),
                                    )
                                    .expect("NZkjgWib____"),
                            )
                            .expect("could not read splice file")
                        })
                        .collect::<Vec<_>>();
                    // NOTE: length is encoded as u64 (same as bincode)
                    let mut data = bincode::serialize(&(iter_size as u64)).expect("a0AAoZik____");
                    data.extend(items.iter().flatten());
                    #[cfg(debug_assertions)]
                    println!("splice | full | {:?}", field);
                    input.__mutate(
                        &mut MutationType::Splice(&mut data.as_slice()),
                        &mut self.visitor.borrow_mut(),
                        path,
                    );
                }
            }
        } else {
            if let Some(possible_splices) = metadata.get_inputs_for_type(ty) {
                let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                let random_splice = possible_splices
                    .get(
                        self.visitor
                            .borrow_mut()
                            .random_range(0, possible_splices.len() - 1),
                    )
                    .unwrap();
                let data = std::fs::read(random_splice).unwrap();
                #[cfg(debug_assertions)]
                println!("splice | one | {:?} {:?}", field, path);
                input.__mutate(
                    &mut MutationType::Splice(&mut data.as_slice()),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
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

impl<I> Named for AutarkieSpliceMutator<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("AutarkieSpliceMutator")
    }
}
impl<I> AutarkieSpliceMutator<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>, max_subslice_size: usize) -> Self {
        Self {
            visitor,
            max_subslice_size,
            phantom: PhantomData,
        }
    }
}
