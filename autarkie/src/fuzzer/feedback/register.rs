use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    marker::PhantomData,
    rc::Rc,
};

use libafl::{
    corpus::{Corpus, Testcase},
    executors::ExitKind,
    feedbacks::{Feedback, StateInitializer},
    inputs::InputToBytes,
    state::{HasCorpus, HasCurrentTestcase},
    Error, HasMetadata,
};

use crate::{
    fuzzer::{context::MutationMetadata, stages::stats::AutarkieStats},
    Node, Visitor,
};
use libafl_bolts::Named;

use crate::fuzzer::context::Context;

pub struct RegisterFeedback<I, TC> {
    bytes_converter: TC,
    visitor: Rc<RefCell<Visitor>>,
    is_solution: bool,
    phantom: PhantomData<I>,
}

impl<I, TC> RegisterFeedback<I, TC>
where
    TC: InputToBytes<I> + Clone,
{
    pub fn new(visitor: Rc<RefCell<Visitor>>, bytes_converter: TC, is_solution: bool) -> Self {
        Self {
            bytes_converter,
            visitor,
            is_solution,
            phantom: PhantomData,
        }
    }
}

impl<I, TC, EM, OT, S> Feedback<EM, I, OT, S> for RegisterFeedback<I, TC>
where
    I: Node,
    TC: InputToBytes<I> + Clone,
    S: HasCurrentTestcase<I> + HasCorpus<I> + HasMetadata,
{
    fn is_interesting(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &I,
        _observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error> {
        Ok(false)
    }

    fn append_metadata(
        &mut self,
        state: &mut S,
        _manager: &mut EM,
        _observers: &OT,
        testcase: &mut Testcase<I>,
    ) -> Result<(), Error> {
        let metadata = state
            .metadata_mut::<Context>()
            .expect("we must have context!");
        metadata.register_input(
            testcase.input().as_ref().expect("we must have input!"),
            &mut self.visitor.borrow_mut(),
            &mut self.bytes_converter,
            self.is_solution,
        );
        let done_mutations = metadata.clear_mutations();
        let metadata = state
            .metadata_mut::<AutarkieStats>()
            .unwrap()
            .add_new_input_mutations(done_mutations);
        Ok(())
    }
}

impl<I, TC, S> StateInitializer<S> for RegisterFeedback<I, TC> {}

impl<I, TC> Named for RegisterFeedback<I, TC> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("RegisterFeedback")
    }
}
