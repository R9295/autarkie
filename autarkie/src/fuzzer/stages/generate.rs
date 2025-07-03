use crate::{fuzzer::context::Context, Node, Visitor};
use libafl::{
    corpus::Corpus,
    events::EventFirer,
    executors::Executor,
    stages::{Restartable, Stage},
    state::{HasCorpus, HasCurrentTestcase},
    Evaluator, HasMetadata,
};
use serde::Serialize;
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

#[derive(Debug)]
pub struct GenerateStage<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I> GenerateStage<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<E, EM, Z, S, I> Stage<E, EM, S, Z> for GenerateStage<I>
where
    I: Node + Serialize,
    S: HasCurrentTestcase<I> + HasCorpus<I> + HasMetadata,
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
        let mut metadata = state.metadata_mut::<Context>()?;
        metadata.generated_input();
        let Some(generated) = generate(&mut self.visitor.borrow_mut()) else {
            metadata.default_input();
            return Ok(());
        };
        metadata.add_mutation(crate::fuzzer::context::MutationMetadata::Generate);
        fuzzer.evaluate_input(state, executor, manager, &generated)?;
        let mut metadata = state.metadata_mut::<Context>()?;
        metadata.default_input();
        Ok(())
    }
}

pub fn generate<I>(visitor: &mut Visitor) -> Option<I>
where
    I: Node,
{
    I::__autarkie_generate(visitor, &mut visitor.generate_depth(), 0, None)
}

impl<I, S> Restartable<S> for GenerateStage<I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl::Error> {
        Ok(())
    }
}
