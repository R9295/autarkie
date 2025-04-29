//! Stage that wraps mutating stages for stats and cleanup
use crate::fuzzer::Context;
use crate::Visitor;
use core::{marker::PhantomData, time::Duration};
use libafl_bolts::{current_time, Error};
use std::cell::RefCell;
use std::rc::Rc;

use libafl::{
    events::EventFirer,
    executors::Executor,
    mutators::{MutationResult, Mutator},
    stages::{Restartable, Stage},
    state::HasCurrentTestcase,
    Evaluator, HasMetadata,
};

#[derive(Debug)]
pub struct AutarkieMutationalStage<S, M, I> {
    inner: M,
    stack: usize,
    phantom: PhantomData<(I, S)>,
}

impl<S, M, I> AutarkieMutationalStage<S, M, I> {
    /// Create a `AutarkieMutationalStage`
    pub fn new(inner: M, stack: usize) -> Self {
        Self {
            inner,
            stack,
            phantom: PhantomData,
        }
    }
}

impl<E, EM, M, Z, S, I> Stage<E, EM, S, Z> for AutarkieMutationalStage<S, M, I>
where
    E: Executor<EM, I, S, Z>,
    Z: Evaluator<E, EM, I, S>,
    EM: EventFirer<I, S>,
    S: HasMetadata + HasCurrentTestcase<I>,
    M: Mutator<I, S>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), Error> {
        let mut current = state.current_input_cloned().unwrap();
        for i in 0..self.stack {
            if self.inner.mutate(state, &mut current)? == MutationResult::Mutated {
                fuzzer.evaluate_input(state, executor, manager, &current)?;
            }
        }
        Ok(())
    }
}

impl<S, M, I> Restartable<S> for AutarkieMutationalStage<S, M, I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), Error> {
        Ok(())
    }
}
