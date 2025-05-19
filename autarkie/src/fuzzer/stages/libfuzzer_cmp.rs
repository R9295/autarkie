use crate::{MutationType, Node, Visitor};
use libafl::{
    corpus::Corpus,
    events::EventFirer,
    executors::{Executor, HasObservers},
    observers::{AFLppCmpValuesMetadata, CmpValues, CmpValuesMetadata, ObserversTuple},
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

use crate::fuzzer::Context;

#[derive(Debug)]
pub struct LibfuzzerCmplogStage<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I> LibfuzzerCmplogStage<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<E, EM, Z, S, I> Stage<E, EM, S, Z> for LibfuzzerCmplogStage<I>
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
    ) -> Result<(), libafl_bolts::Error> {
        if state.current_testcase().unwrap().scheduled_count() > 0 {
            return Ok(());
        }
        let unmutated_input = state.current_input_cloned()?;
        let mut reduced = HashSet::new();
        if let Ok(data) = state.metadata::<CmpValuesMetadata>() {
            for i in data.list.clone() {
                match i {
                    CmpValues::U16((left, right, is_const)) => {
                        reduced.insert((left as u64, right as u64));
                        reduced.insert((right as u64, left as u64));
                    }
                    CmpValues::U32((left, right, is_const)) => {
                        reduced.insert((left as u64, right as u64));
                        reduced.insert((right as u64, left as u64));
                    }
                    CmpValues::U64((left, right, is_const)) => {
                        reduced.insert((left, right));
                        reduced.insert((right, left));
                    }
                    CmpValues::Bytes((left, right)) => {
                        if left.as_slice()
                            != [
                                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            ]
                            && right.as_slice() != left.as_slice()
                        {
                            // TODO
                        }
                    }
                    // ignore U8
                    CmpValues::U8(_) => {}
                }
            }
        }

        let metadata = state
            .metadata_mut::<Context>()
            .expect("we must have context!");
        metadata.generated_input();
        for cmp in reduced {
            unmutated_input.__autarkie_cmps(&mut self.visitor.borrow_mut(), 0, cmp);
            let matches = self.visitor.borrow_mut().cmps();
            for path in matches {
                let cmp_path = path.0.iter().map(|(i, ty)| i.0).collect::<VecDeque<_>>();
                let mut serialized_alternative = path.1.as_slice();
                let mut input = unmutated_input.clone();
                let before = crate::serialize(&input);
                #[cfg(debug_assertions)]
                println!("cmplog_splice | one | {:?}", path.0);
                input.__autarkie_mutate(
                    &mut MutationType::Splice(&mut serialized_alternative),
                    &mut self.visitor.borrow_mut(),
                    cmp_path,
                );

                let metadata = state
                    .metadata_mut::<Context>()
                    .expect("we must have context!");
                metadata.add_mutation(crate::fuzzer::context::MutationMetadata::Cmplog);
                let res = fuzzer.evaluate_input(state, executor, manager, &input)?;
            }
        }
        let metadata = state
            .metadata_mut::<Context>()
            .expect("we must have context!");
        metadata.default_input();

        // walk all fields in the input and capture the paths where reduced is present and store
        // those paths as potentially interesting.
        Ok(())
    }
}

impl<I, S> Restartable<S> for LibfuzzerCmplogStage<I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl::Error> {
        Ok(())
    }
}
