use std::marker::PhantomData;

use libafl::{
    events::{Event, EventManagerHook},
    state::{State, UsesState},
};

#[derive(Clone, Debug, Copy)]
pub struct NoShare<S> {
    phantom: PhantomData<S>,
}

impl<S> NoShare<S> {
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<S> UsesState for NoShare<S>
where
    S: State,
{
    type State = S;
}

impl<S> EventManagerHook<S> for NoShare<S>
where
    S: State,
{
    fn on_fire(
        &mut self,
        _state: &mut S,
        _client_id: libafl_bolts::ClientId,
        _event: &libafl::events::Event<<S>::Input>,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }

    fn pre_exec(
        &mut self,
        state: &mut S,
        client_id: libafl_bolts::ClientId,
        event: &libafl::events::Event<<S>::Input>,
    ) -> Result<bool, libafl::Error> {
        if let Event::NewTestcase {
            input,
            observers_buf,
            exit_kind,
            corpus_size,
            client_config,
            time,
            forward_id,
        } = event
        {
            return Ok(false);
        } else if let Event::Objective {
            objective_size,
            time,
        } = event
        {
            return Ok(false);
        }
        Ok(true)
    }

    fn post_exec(
        &mut self,
        _state: &mut S,
        _client_id: libafl_bolts::ClientId,
    ) -> Result<bool, libafl::Error> {
        Ok(true)
    }
}
