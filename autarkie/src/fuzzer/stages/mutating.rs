//! Stage that wraps mutating stages for stats and cleanup
use crate::fuzzer::Context;
use crate::Visitor;
use core::{marker::PhantomData, time::Duration};
use libafl_bolts::{current_time, Error};
use std::cell::RefCell;
use std::rc::Rc;

use libafl::{
    stages::{Restartable, Stage},
    HasMetadata,
};
/// Track an inner Stage's execution time
#[derive(Debug)]
pub struct MutatingStageWrapper<S, ST> {
    inner: ST,
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<S>,
}

impl<S, ST> MutatingStageWrapper<S, ST> {
    /// Create a `MutatingStageWrapper`
    pub fn new(inner: ST, visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            inner,
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<E, M, Z, S, ST> Stage<E, M, S, Z> for MutatingStageWrapper<S, ST>
where
    S: HasMetadata,
    ST: Stage<E, M, S, Z>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut M,
    ) -> Result<(), Error> {
        self.inner.perform(fuzzer, executor, state, manager)?;
        let _ = state.metadata_mut::<Context>().unwrap().clear_mutations();
        let _ = self.visitor.borrow_mut().serialized();
        Ok(())
    }
}

impl<S, ST> Restartable<S> for MutatingStageWrapper<S, ST>
where
    ST: Restartable<S>,
{
    fn should_restart(&mut self, state: &mut S) -> Result<bool, Error> {
        self.inner.should_restart(state)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), Error> {
        self.inner.clear_progress(state)
    }
}
