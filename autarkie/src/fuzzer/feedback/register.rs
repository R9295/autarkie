/// Benign feedback to capture all serialized values of nodes and store them in the corpora
use std::{borrow::Cow, cell::RefCell, marker::PhantomData, rc::Rc};

use libafl::{
    corpus::{Corpus, Testcase},
    executors::ExitKind,
    feedbacks::{Feedback, StateInitializer},
    state::{HasCorpus, HasCurrentTestcase, State},
    Error, HasMetadata,
};

use crate::{Node, Visitor};
use libafl_bolts::Named;

use crate::fuzzer::Context;

pub struct RegisterFeedback<I> {
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<I>,
}

impl<I> RegisterFeedback<I> {
    pub fn new(visitor: Rc<RefCell<Visitor>>) -> Self {
        Self {
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<I, EM, OT, S> Feedback<EM, I, OT, S> for RegisterFeedback<I>
where
    I: Node,
    S: State + HasCurrentTestcase + HasCorpus + HasMetadata,
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

    fn discard_metadata(&mut self, _state: &mut S, _input: &I) -> Result<(), Error> {
        Ok(())
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
        );
        Ok(())
    }
}

impl<I, S> StateInitializer<S> for RegisterFeedback<I> {}

impl<I> Named for RegisterFeedback<I> {
    fn name(&self) -> &std::borrow::Cow<'static, str> {
        &Cow::Borrowed("RegisterFeedback")
    }
}
