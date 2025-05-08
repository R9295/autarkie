//! Stage that wraps mutating stages for stats and cleanup
use crate::Visitor;
use crate::{fuzzer::Context, Node};
use core::{marker::PhantomData, time::Duration};
use libafl::inputs::BytesInput;
use libafl::mutators::MutatorsTuple;
use libafl::state::HasRand;
use libafl_bolts::rands::Rand;
use libafl_bolts::{current_time, Error};
use std::cell::RefCell;
use std::num::NonZero;
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
pub struct AutarkieAflStage<S, M, I> {
    inner: M,
    stack: usize,
    phantom: PhantomData<(I, S)>,
}

impl<S, M, I> AutarkieAflStage<S, M, I> {
    /// Create a `AutarkieAflStage`
    pub fn new(inner: M, stack: usize) -> Self {
        Self {
            stack,
            inner,
            phantom: PhantomData,
        }
    }
}

impl<E, EM, M, Z, S, I> Stage<E, EM, S, Z> for AutarkieAflStage<S, M, I>
where
    I: Node,
    E: Executor<EM, I, S, Z>,
    Z: Evaluator<E, EM, I, S>,
    EM: EventFirer<I, S>,
    S: HasMetadata + HasCurrentTestcase<I> + HasRand,
    M: MutatorsTuple<Vec<u8>, S>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), Error> {
        let mut metadata = state.metadata_mut::<Context>().expect("fxeZamEw____");
        metadata.generated_input();
        let mut input = crate::serialize(&state.current_input_cloned().unwrap());
        for _ in 0..self.stack {
            let mutation = state
                .rand_mut()
                .below(unsafe { NonZero::new(self.inner.len()).unwrap_unchecked() })
                .into();
            if self.inner.get_and_mutate(mutation, state, &mut input)? == MutationResult::Mutated {
                #[cfg(feature = "scale")]
                let Some(deserialized) = crate::maybe_deserialize(&mut input.as_slice()) else {
                    return Ok(());
                };
                #[cfg(not(feature = "scale"))]
                let Some(deserialized) = crate::maybe_deserialize(&mut input.as_slice()) else {
                    return Ok(());
                };
                let mut metadata = state.metadata_mut::<Context>().unwrap();
                metadata.add_mutation(crate::fuzzer::context::MutationMetadata::Afl);
                fuzzer.evaluate_input(state, executor, manager, &deserialized)?;
            }
        }
        let mut metadata = state.metadata_mut::<Context>().expect("fxeZamEw____");
        metadata.default_input();
        Ok(())
    }
}

impl<S, M, I> Restartable<S> for AutarkieAflStage<S, M, I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), Error> {
        Ok(())
    }
}
