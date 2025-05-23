use crate::{MutationType, Node, Visitor};
use libafl::{
    corpus::Corpus,
    events::EventFirer,
    executors::{Executor, HasObservers},
    observers::{AFLppCmpValuesMetadata, CmpValues, ObserversTuple},
    stages::{Restartable, Stage},
    state::HasCurrentTestcase,
    Evaluator, HasMetadata,
};
use libafl_bolts::{
    tuples::{Handle, MatchNameRef},
    AsSlice,
};
use libafl_targets::AFLppCmpLogObserver;
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    marker::PhantomData,
    rc::Rc,
};

use crate::fuzzer::Context;

#[derive(Debug)]
pub struct CmpLogStage<'a, TE, I> {
    visitor: Rc<RefCell<Visitor>>,
    tracer_executor: TE,
    cmplog_observer_handle: Handle<AFLppCmpLogObserver<'a>>,
    phantom: PhantomData<I>,
}

impl<'a, TE, I> CmpLogStage<'a, TE, I> {
    pub fn new(
        visitor: Rc<RefCell<Visitor>>,
        tracer_executor: TE,
        cmplog_observer_handle: Handle<AFLppCmpLogObserver<'a>>,
    ) -> Self {
        Self {
            cmplog_observer_handle,
            tracer_executor,
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<TE, E, EM, Z, S, I> Stage<E, EM, S, Z> for CmpLogStage<'_, TE, I>
where
    I: Node + Serialize + Clone,
    S: HasCurrentTestcase<I> + HasMetadata,
    E: Executor<EM, I, S, Z>,
    EM: EventFirer<I, S>,
    TE: Executor<EM, I, S, Z> + HasObservers,
    TE::Observers: MatchNameRef + ObserversTuple<I, S>,
    Z: Evaluator<E, EM, I, S>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), libafl_bolts::Error> {
        if state.current_testcase().unwrap().scheduled_count() > 1 {
            return Ok(());
        }

        let unmutated_input = state.current_input_cloned()?;

        let mut obs = self.tracer_executor.observers_mut();
        let ob = obs.get_mut(&self.cmplog_observer_handle).unwrap();
        ob.set_original(true);
        self.tracer_executor
            .observers_mut()
            .pre_exec_all(state, &unmutated_input)?;

        let exit_kind =
            self.tracer_executor
                .run_target(fuzzer, state, manager, &unmutated_input)?;
        self.tracer_executor
            .observers_mut()
            .post_exec_all(state, &unmutated_input, &exit_kind)?;

        let mut reduced = HashSet::new();
        if let Ok(data) = state.metadata::<AFLppCmpValuesMetadata>() {
            for item in data.orig_cmpvals().values() {
                for i in item.iter() {
                    match i {
                        CmpValues::U16((left, right, is_const)) => {
                            reduced.insert((*left as u64, *right as u64));
                            reduced.insert((*right as u64, *left as u64));
                        }
                        CmpValues::U32((left, right, is_const)) => {
                            reduced.insert((*left as u64, *right as u64));
                            reduced.insert((*right as u64, *left as u64));
                        }
                        CmpValues::U64((left, right, is_const)) => {
                            reduced.insert((*left, *right));
                            reduced.insert((*right, *left));
                        }
                        CmpValues::Bytes((left, right)) => {
                            // TODO
                        }
                        // ignore U8
                        CmpValues::U8(_) => {}
                    }
                }
            }
        }

        let metadata = state
            .metadata_mut::<Context>()
            .expect("we must have context!");

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
                let res = fuzzer.evaluate_input(state, executor, manager, &input)?;
            }
        }

        // walk all fields in the input and capture the paths where reduced is present and store
        // those paths as potentially interesting.
        Ok(())
    }
}

impl<'a, TE, I, S> Restartable<S> for CmpLogStage<'a, TE, I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl::Error> {
        Ok(())
    }
}
