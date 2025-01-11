use std::{borrow::Cow, marker::PhantomData};

use libafl::{
    corpus::{Corpus, Testcase},
    executors::ExitKind,
    feedbacks::{Feedback, StateInitializer},
    state::{HasCorpus, HasCurrentTestcase, State},
    Error, HasMetadata,
};

use libafl_bolts::Named;
use thesis::Node;

use crate::context::Context;

pub struct RegisterFeedback<I> {
    phantom: PhantomData<I>,
}

impl<I> RegisterFeedback<I> {
    pub fn new() -> Self {
        Self {
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
        let id = state.corpus().peek_free_id();
        let corpus_id = id;
        let metadata = state
            .metadata_mut::<Context>()
            .expect("we must have context!");
        metadata.register_input(
            testcase.input().as_ref().expect("we must have input!"),
            corpus_id,
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
