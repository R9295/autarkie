use crate::{fuzzer::context::MutationMetadata, MutationType, Node, Visitor};
#[cfg(feature = "afl")]
use libafl::observers::{AflppCmpValuesMetadata, CmpValues, ObserversTuple};
#[cfg(feature = "libfuzzer")]
use libafl::observers::{CmpValues, CmpValuesMetadata, ObserversTuple};
use libafl::{
    corpus::Corpus,
    events::EventFirer,
    executors::{Executor, HasObservers},
    stages::{Restartable, Stage},
    state::HasCurrentTestcase,
    Evaluator, HasMetadata,
};
use libafl_bolts::{
    tuples::{Handle, MatchNameRef},
    AsSlice,
};
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    marker::PhantomData,
    rc::Rc,
};

use crate::fuzzer::context::Context;

#[derive(Debug)]
pub struct AutarkieCmpLogStage<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I> AutarkieCmpLogStage<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<E, EM, Z, S, I> Stage<E, EM, S, Z> for AutarkieCmpLogStage<I>
where
    I: Node + Serialize + Clone,
    S: HasCurrentTestcase<I> + HasMetadata,
    E: Executor<EM, I, S, Z>,
    EM: EventFirer<I, S>,
    Z: Evaluator<E, EM, I, S>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), libafl::Error> {
        if state.current_testcase().unwrap().scheduled_count() > 0 {
            return Ok(());
        }
        let mut unmutated_input = state.current_input_cloned()?;
        let mut reduced = HashSet::new();
        let mut reduced_bytes = HashSet::new();
        #[cfg(feature = "afl")]
        let data = {
            let Ok(data) = state.metadata::<AflppCmpValuesMetadata>() else {
                return Ok(());
            };
            data.orig_cmpvals().values()
        };
        #[cfg(feature = "libfuzzer")]
        let data = {
            let Ok(data) = state.metadata::<CmpValuesMetadata>() else {
                return Ok(());
            };
            &data.list
        };
        for item in data {
            for i in item.into_iter() {
                if let Some((left, right, _is_const)) = i.to_u64_tuple() {
                    reduced.insert((left, right));
                } else {
                    if let CmpValues::Bytes((left, right)) = i {
                        reduced_bytes.insert(left.as_slice().to_vec());
                        reduced_bytes.insert(right.as_slice().to_vec());
                    }
                }
            }
        }
        let mut unmutated_input_bytes = crate::serialize(&unmutated_input);
        for cmp_chunk in reduced_bytes {
            let mut start = None;
            while let Some(index) = find_subsequence(&unmutated_input_bytes, &cmp_chunk, start) {
                let mut cloned = unmutated_input_bytes.clone();
                cloned.splice(index..index + cmp_chunk.len(), cmp_chunk.to_vec());
                start = Some(index + cmp_chunk.len());
                #[cfg(feature = "bincode")]
                let Some(deserialized) = crate::maybe_deserialize(&cloned) else {
                    continue;
                };
                #[cfg(not(feature = "bincode"))]
                let Some(deserialized) = crate::maybe_deserialize(&mut cloned.as_slice()) else {
                    continue;
                };
                unmutated_input_bytes = cloned;
                state.metadata_mut::<Context>().unwrap().generated_input();
                state
                    .metadata_mut::<Context>()
                    .unwrap()
                    .add_mutation(MutationMetadata::CmplogBytes);
                let res = fuzzer.evaluate_input(state, executor, manager, &deserialized)?;
                state.metadata_mut::<Context>().unwrap().default_input();
            }
        }

        for cmp in reduced {
            unmutated_input.__autarkie_cmps(&mut self.visitor.borrow_mut(), 0, cmp);
            let matches = self.visitor.borrow_mut().cmps();
            for path in matches {
                let cmp_path = path.0.iter().map(|(i, ty)| i.0).collect::<VecDeque<_>>();
                let mut serialized_alternative = path.1.as_slice();
                state
                    .metadata_mut::<Context>()
                    .unwrap()
                    .add_mutation(MutationMetadata::Cmplog);
                #[cfg(debug_assertions)]
                println!("cmplog_splice | one | {:?}", path.0);
                unmutated_input.__autarkie_mutate(
                    &mut MutationType::Splice(&mut serialized_alternative),
                    &mut self.visitor.borrow_mut(),
                    cmp_path,
                );
                fuzzer.evaluate_input(state, executor, manager, &unmutated_input)?;
            }
        }

        Ok(())
    }
}

impl<I, S> Restartable<S> for AutarkieCmpLogStage<I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl::Error> {
        Ok(())
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8], start: Option<usize>) -> Option<usize> {
    haystack[start.unwrap_or(0)..]
        .windows(needle.len())
        .position(|window| window == needle)
}
