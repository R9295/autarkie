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

pub struct AutarkieSpliceMutator<I> {
    visitor: Rc<RefCell<Visitor>>,
    max_subslice_size: usize,
    file_cache: FileCache,
    phantom: PhantomData<I>,
}


impl<I, S> Mutator<I, S> for AutarkieSpliceMutator<I>
where
    I: Node,
    S: HasCorpus<I> + HasRand + HasMetadata,
{
    fn mutate(&mut self, state: &mut S, input: &mut I) -> Result<MutationResult, libafl::Error> {
        let mut metadata = state.metadata_mut::<Context>()?;
        input.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();
        let field_splice_index = self.visitor.borrow_mut().random_range(0, fields.len() - 1);
        let field = &fields[field_splice_index];
        let ((id, node_ty), ty) = field.last().expect("EfxPNdQ0____");
        if let crate::NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
            let subslice = self.visitor.borrow_mut().coinflip_with_prob(0.6);
            if subslice && *field_len > 3 {
                let Some(possible_splices) = metadata.get_inputs_for_type(&inner_ty) else {
                    return Ok(MutationResult::Skipped);
                };
                let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
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
                        .expect("BCUHhFol____");
                    let data = self
                        .file_cache
                        .read_cached(random_splice)
                        .expect("4phGbftw____");
                    #[cfg(feature = "debug_mutators")]
                    println!("splice | subslice | {:?}", (&field, &path));
                    input.__autarkie_mutate(
                        &mut MutationType::Splice(&mut data.as_slice()),
                        &mut self.visitor.borrow_mut(),
                        child_path,
                    );
                }
                metadata.add_mutation(crate::fuzzer::context::MutationMetadata::SpliceSubSplice);
            } else {
                let Some(possible_splices) = metadata.get_inputs_for_type(&inner_ty) else {
                    return Ok(MutationResult::Skipped);
                };
                let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                let mut data = if !*is_fixed_len {
                    crate::serialize_vec_len(if *field_len > 0 { *field_len } else { 0 })
                } else {
                    vec![]
                };
                // unfortunately we need to replace the exact amount.
                // cause we don't differentiate between vec and slice
                for _ in (0..*field_len) {
                    let path = possible_splices
                        .get(
                            self.visitor
                                .borrow_mut()
                                .random_range(0, possible_splices.len() - 1),
                        )
                        .expect("NZkjgWib____");
                    data.extend_from_slice(
                        self.file_cache.read_cached(path).expect("____gJaxjQmU"),
                    );
                }
                #[cfg(feature = "debug_mutators")]
                println!("splice | full | {:?}", field);
                input.__autarkie_mutate(
                    &mut MutationType::Splice(&mut data.as_slice()),
                    &mut self.visitor.borrow_mut(),
                    path,
                );
                metadata.add_mutation(crate::fuzzer::context::MutationMetadata::SpliceFull);
            }
        } else {
            let Some(possible_splices) = metadata.get_inputs_for_type(ty) else {
                return Ok(MutationResult::Skipped);
            };
            let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            let random_splice = possible_splices
                .get(
                    self.visitor
                        .borrow_mut()
                        .random_range(0, possible_splices.len() - 1),
                )
                .expect("____zyUpz0uu");
            let data = self
                .file_cache
                .read_cached(random_splice)
                .expect("____gJaxjQmU");
            #[cfg(feature = "debug_mutators")]
            println!("splice | one | {:?} {:?}", field, path);
            input.__autarkie_mutate(
                &mut MutationType::Splice(&mut data.as_slice()),
                &mut self.visitor.borrow_mut(),
                path,
            );
            metadata.add_mutation(crate::fuzzer::context::MutationMetadata::SpliceSingle);
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
            file_cache: FileCache::new(256),
            phantom: PhantomData,
        }
    }
}
