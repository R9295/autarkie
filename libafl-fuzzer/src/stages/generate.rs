use libafl::{
    corpus::Corpus,
    executors::Executor,
    inputs::UsesInput,
    stages::Stage,
    state::{HasCorpus, HasCurrentTestcase, State, UsesState},
    Evaluator,
};
use serde::Serialize;
use std::{cell::RefCell, marker::PhantomData, rc::Rc};
use thesis::{Node, Visitor};

#[derive(Debug)]
pub struct GenerateStage<E, S, I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<(E, S, I)>,
}

impl<E, S, I> GenerateStage<E, S, I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<E, S, I> UsesState for GenerateStage<E, S, I>
where
    S: State,
{
    type State = S;
}

impl<E, EM, Z, S, I> Stage<E, EM, Z> for GenerateStage<E, S, I>
where
    I: Node + Serialize,
    S: State + HasCurrentTestcase + HasCorpus + UsesInput<Input = I>,
    S::Corpus: Corpus<Input = I>,
    E: UsesState<State = S> + Executor<E, EM, State = S>,
    EM: UsesState<State = S>,
    Z: UsesState<State = S> + Evaluator<E, EM>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut Self::State,
        manager: &mut EM,
    ) -> Result<(), libafl_bolts::Error> {
        let generated = generate(&mut self.visitor.borrow_mut());
        fuzzer.evaluate_input(state, executor, manager, generated)?;
        Ok(())
    }

    fn should_restart(&mut self, state: &mut Self::State) -> Result<bool, libafl_bolts::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut Self::State) -> Result<(), libafl_bolts::Error> {
        Ok(())
    }
}

pub fn generate<I>(visitor: &mut Visitor) -> I
where
    I: Node,
{
    I::generate(visitor, &mut visitor.generate_depth(), &mut 0)
}
