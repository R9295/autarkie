use crate::{fuzzer::context::MutationMetadata, MutationType, Node, Visitor};
#[cfg(feature = "afl")]
use libafl::observers::AflppCmpValuesMetadata;
#[cfg(any(feature = "libfuzzer", feature = "llvm-fuzzer-no-link"))]
use libafl::observers::CmpValuesMetadata;
use libafl::observers::{CmpValues, ObserversTuple};
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
        {
            let Ok(data) = state.metadata::<AflppCmpValuesMetadata>() else {
                return Ok(());
            };
            let values = data.orig_cmpvals().values();
            for item in values {
                for i in item.into_iter() {
                    if let Some((left, right, _is_const)) = i.to_u64_tuple() {
                        reduced.insert((left, right));
                    } else {
                        if let CmpValues::Bytes((left, right)) = i {
                            reduced_bytes
                                .insert((left.as_slice().to_vec(), right.as_slice().to_vec()));
                        }
                    }
                }
            }
        };
        #[cfg(any(feature = "libfuzzer", feature = "llvm-fuzzer-no-link"))]
        {
            let Ok(data) = state.metadata::<CmpValuesMetadata>() else {
                return Ok(());
            };
            for i in data.list.iter() {
                if let Some((left, right, _is_const)) = i.to_u64_tuple() {
                    reduced.insert((left, right));
                } else {
                    if let CmpValues::Bytes((left, right)) = i {
                        reduced_bytes.insert((left.as_slice().to_vec(), right.as_slice().to_vec()));
                    }
                }
            }
        };
        let original_bytes = crate::serialize(&unmutated_input);
        for (left, right) in reduced_bytes {
            for (needle, replacement) in [(&left, &right), (&right, &left)] {
                if needle.is_empty() {
                    continue;
                }
                let mut start = None;
                while let Some(index) = find_subsequence(&original_bytes, needle, start) {
                    start = Some(index + needle.len());
                    let mut cloned = original_bytes.clone();
                    cloned.splice(index..index + needle.len(), replacement.iter().copied());
                    #[cfg(feature = "bincode")]
                    let Some(deserialized) = crate::maybe_deserialize(&cloned) else {
                        continue;
                    };
                    #[cfg(not(feature = "bincode"))]
                    let Some(deserialized) = crate::maybe_deserialize(&mut cloned.as_slice()) else {
                        continue;
                    };
                    state.metadata_mut::<Context>().unwrap().generated_input();
                    state
                        .metadata_mut::<Context>()
                        .unwrap()
                        .add_mutation(MutationMetadata::CmplogBytes);
                    let res = fuzzer.evaluate_input(state, executor, manager, &deserialized)?;
                    state.metadata_mut::<Context>().unwrap().default_input();
                }
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
                let mut candidate = unmutated_input.clone();
                candidate.__autarkie_mutate(
                    &mut MutationType::Splice(&mut serialized_alternative),
                    &mut self.visitor.borrow_mut(),
                    cmp_path,
                );
                fuzzer.evaluate_input(state, executor, manager, &candidate)?;
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
    if needle.is_empty() {
        return None;
    }
    let start = start.unwrap_or(0);
    if start > haystack.len() {
        return None;
    }
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|relative| start + relative)
}

#[cfg(test)]
mod tests {
    use super::find_subsequence;

    #[test]
    fn returns_absolute_offsets_for_repeated_needle() {
        let haystack = b"abXYabXYab";
        let needle = b"ab";
        let first = find_subsequence(haystack, needle, None).expect("first");
        assert_eq!(first, 0);
        let second =
            find_subsequence(haystack, needle, Some(first + needle.len())).expect("second");
        assert_eq!(second, 4);
        let third =
            find_subsequence(haystack, needle, Some(second + needle.len())).expect("third");
        assert_eq!(third, 8);
        assert_eq!(
            find_subsequence(haystack, needle, Some(third + needle.len())),
            None
        );
    }

    #[test]
    fn empty_needle_is_none_not_panic() {
        assert_eq!(find_subsequence(b"anything", b"", None), None);
    }

    #[test]
    fn start_past_end_is_none() {
        assert_eq!(find_subsequence(b"ab", b"ab", Some(99)), None);
    }
}
