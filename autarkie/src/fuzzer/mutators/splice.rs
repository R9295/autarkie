use crate::Visitor;
use crate::{MutationType, Node};
use libafl::monitors::PerfFeature;
use libafl::{
    corpus::Corpus,
    mutators::{MutationResult, Mutator},
    state::{HasCorpus, HasRand, State},
    HasMetadata,
};
#[cfg(feature = "introspection")]
use libafl::{mark_feature_time, start_timer};
use libafl_bolts::{current_time, AsSlice, Named};
use std::{borrow::Cow, cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use crate::fuzzer::Context;

use super::commons::calculate_subslice_bounds;

pub struct AutarkieSpliceMutator<I> {
    visitor: Rc<RefCell<Visitor>>,
    max_subslice_size: usize,
    phantom: PhantomData<I>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Data {
    Fields,
}

impl<I, S> Mutator<I, S> for AutarkieSpliceMutator<I>
where
    I: Node,
    S: State + HasCorpus + HasRand + HasMetadata,
    S::Corpus: Corpus<Input = I>,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        let mut metadata = state.metadata_mut::<Context>()?;
        let mut mutated_path = None;
        #[cfg(feature = "introspection")]
        start_timer!(state);
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        #[cfg(feature = "introspection")]
        mark_feature_time!(state, Data::Fields);
        let mut fields = self.visitor.borrow_mut().fields();
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &fields[field_splice_index];
        let ((id, node_ty), ty) = field.last().unwrap();
        if let crate::NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
            let subslice = self.visitor.borrow_mut().coinflip_with_prob(0.6);
            if subslice {
                // no point subslicing when we have less than 5 entries
                if *field_len < 3 {
                    return Ok(MutationResult::Skipped);
                }
                if let Some(possible_splices) = metadata.get_inputs_for_type(&inner_ty) {
                    let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                    mutated_path = Some(path.clone());
                    let subslice_bounds = calculate_subslice_bounds(
                        *field_len,
                        self.max_subslice_size,
                        &mut self.visitor.borrow_mut(),
                    );
                    for index in subslice_bounds {
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
                        input.__autarkie_mutate(
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
                    let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                    let items = (0..*field_len)
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
                    let mut data = if !*is_fixed_len {
                        crate::serialize_vec_len(if *field_len > 0 { *field_len } else { 0 })
                    } else {
                        vec![]
                    };
                    data.extend(items.iter().flatten());
                    mutated_path = Some(path.clone());
                    #[cfg(debug_assertions)]
                    println!("splice | full | {:?}", field);
                    input.__autarkie_mutate(
                        &mut MutationType::Splice(&mut data.as_slice()),
                        &mut self.visitor.borrow_mut(),
                        path,
                    );
                } else {
                    return Ok(MutationResult::Skipped);
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
                mutated_path = Some(path.clone());
                #[cfg(debug_assertions)]
                println!("splice | one | {:?} {:?}", field, path);
                input.__autarkie_mutate(
                    &mut MutationType::Splice(&mut data.as_slice()),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
            } else {
                return Ok(MutationResult::Skipped);
            };
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
