use libafl::{
    corpus::Corpus,
    executors::Executor,
    inputs::UsesInput,
    stages::Stage,
    state::{HasCorpus, HasCurrentTestcase, State, UsesState},
    Evaluator,
};
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    marker::PhantomData,
    rc::Rc,
};
use thesis::{Node, Visitor};

#[derive(Debug)]
pub struct DeterministicStage<E, S, I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<(E, S, I)>,
}

impl<E, S, I> DeterministicStage<E, S, I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<E, S, I> UsesState for DeterministicStage<E, S, I>
where
    S: State,
{
    type State = S;
}

impl<E, EM, Z, S, I> Stage<E, EM, Z> for DeterministicStage<E, S, I>
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
        if state.current_testcase().unwrap().scheduled_count() > 1 {
            return Ok(());
        }
        let input = state.current_input_cloned()?;
        input.fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();
        // generate replace every field
        for field in fields {
            let mut unmutated_input = state.current_input_cloned()?;
            let mut path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
            unmutated_input.__mutate(
                &mut thesis::MutationType::GenerateReplace(3),
                &mut self.visitor.borrow_mut(), 
                path
            );
            let res = fuzzer.evaluate_input(state, executor, manager, unmutated_input)?;
            #[cfg(debug_assertions)]
            if let libafl::ExecuteInputResult::Corpus = res.0 {
                println!("FOUND USING DETERMINISTIC");
            }
        }
        Ok(())
    }

    fn should_restart(&mut self, state: &mut Self::State) -> Result<bool, libafl_bolts::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut Self::State) -> Result<(), libafl_bolts::Error> {
        Ok(())
    }
}
