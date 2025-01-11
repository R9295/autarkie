use libafl::{
    corpus::Corpus,
    executors::{Executor, HasObservers},
    inputs::UsesInput,
    observers::{AFLppCmpValuesMetadata, CmpValues, ObserversTuple},
    stages::Stage,
    state::{HasCurrentTestcase, State, UsesState},
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
use thesis::{MutationType, Node, Visitor};

use crate::context::Context;

#[derive(Debug)]
pub struct CmpLogStage<'a, TE, E, S, I> {
    visitor: Rc<RefCell<Visitor>>,
    tracer_executor: TE,
    cmplog_observer_handle: Handle<AFLppCmpLogObserver<'a>>,
    phantom: PhantomData<(E, S, I)>,
}

impl<'a, TE, E, S, I> CmpLogStage<'a, TE, E, S, I> {
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

impl<TE, E, S, I> UsesState for CmpLogStage<'_, TE, E, S, I>
where
    S: State,
{
    type State = S;
}

impl<TE, E, EM, Z, S, I> Stage<E, EM, Z> for CmpLogStage<'_, TE, E, S, I>
where
    I: Node + Serialize + Clone,
    S: State + HasCurrentTestcase + HasMetadata + UsesInput<Input = I>,
    S::Corpus: Corpus<Input = I>,
    E: UsesState<State = S> + Executor<E, EM, State = S>,
    EM: UsesState<State = S>,
    TE: Executor<EM, Z, State = S> + HasObservers,
    TE::Observers: MatchNameRef + ObserversTuple<I, TE::State>,
    Z: UsesState<State = S> + Evaluator<E, EM>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut Self::State,
        manager: &mut EM,
    ) -> Result<(), libafl_bolts::Error> {
        if state.current_testcase().unwrap().scheduled_count() > 1 {
            return Ok(());
        }
        // First run with the un-mutated input
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
        // TODO: store interesting paths
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
                            /* if left.as_slice()
                                != [
                                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                                ]
                                && right.as_slice() != left.as_slice()
                            {
                                panic!("{:?} {:?}", right, left);
                            } */
                        }
                        // ignore u8
                        _ => {}
                    }
                }
            }
        }
        let metadata = state
            .metadata_mut::<Context>()
            .expect("we must have context!");
        for cmp in reduced {
            unmutated_input.cmps(&mut self.visitor.borrow_mut(), 0, cmp);
            let matches = self.visitor.borrow_mut().cmps();
            for path in matches {
                let cmp_path = path.0.iter().map(|(i, ty)| i.0).collect::<VecDeque<_>>();
                let mut serialized_alternative = path.1.as_slice();
                let mut input = unmutated_input.clone();
                let before = thesis::serialize(&input);
                #[cfg(debug_assertions)]
                println!("cmplog_splice | one | {:?}", path.0);
                input.__mutate(&mut MutationType::Splice(&mut serialized_alternative), &mut self.visitor.borrow_mut(), cmp_path);
                let res = fuzzer.evaluate_input(state, executor, manager, input)?;
/*                 #[cfg(debug_assertions)] */
                if let libafl::ExecuteInputResult::Corpus = res.0 {
                    println!("FOUND USING CMPLOG");
                }
            }
        }

        // walk all fields in the input and capture the paths where reduced is present and store
        // those paths as potentially interesting.
        Ok(())
    }

    fn should_restart(&mut self, state: &mut Self::State) -> Result<bool, libafl_bolts::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut Self::State) -> Result<(), libafl_bolts::Error> {
        Ok(())
    }
}
