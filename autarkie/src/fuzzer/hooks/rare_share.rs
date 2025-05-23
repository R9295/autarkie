use libafl::{
    events::{Event, EventManagerHook, EventWithStats},
    Error,
};

#[derive(Debug, Clone, Copy)]
pub struct RareShare {
    skip_count: usize,
    skipped: usize,
}

impl<I, S> EventManagerHook<I, S> for RareShare {
    fn pre_receive(
        &mut self,
        state: &mut S,
        client_id: libafl_bolts::ClientId,
        event: &EventWithStats<I>,
    ) -> Result<bool, Error> {
        if matches!(event.event(), Event::NewTestcase { .. }) {
            if self.skipped == self.skip_count {
                self.skipped = 0;
                return Ok(false);
            } else {
                self.skipped += 1;
                return Ok(true);
            }
        }
        Ok(true)
    }
}

impl RareShare {
    pub fn new(skip_count: usize) -> Self {
        Self {
            skip_count,
            skipped: 0,
        }
    }
}
