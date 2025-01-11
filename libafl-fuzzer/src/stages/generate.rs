use autarkie::{Node, Visitor};
use libafl::{
    corpus::Corpus,
    executors::Executor,
    stages::Stage,
    state::{HasCorpus, HasCurrentTestcase, State, UsesState},
    Evaluator,
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
    S: State + HasCurrentTestcase + HasCorpus,
    S::Corpus: Corpus<Input = I>,
    E: Executor<EM, I, S, Z>,
    EM: UsesState<State = S>,
    Z: Evaluator<E, EM, I, S>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), libafl_bolts::Error> {
        let generated = generate(&mut self.visitor.borrow_mut());
        fuzzer.evaluate_input(state, executor, manager, generated)?;
        Ok(())
    }

    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl_bolts::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl_bolts::Error> {
        Ok(())
    }
}

pub fn generate<I>(visitor: &mut Visitor) -> I
where
    I: Node,
{
    I::generate(visitor, &mut visitor.generate_depth(), &mut 0)
}
