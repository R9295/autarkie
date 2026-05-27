use std::{
    borrow::Cow,
    marker::PhantomData,
    num::NonZero,
    path::{Path, PathBuf},
};

use libafl::{
    corpus::{Corpus, CorpusId, Testcase},
    executors::ExitKind,
    feedbacks::{Feedback, StateInitializer},
    inputs::{Input, ToTargetBytes},
    observers::MapObserver,
    schedulers::Scheduler,
    state::{HasCorpus, HasRand},
    Error, HasMetadata,
};
use libafl_bolts::{
    rands::Rand,
    tuples::{Handle, Handled, MatchName, MatchNameRef},
    AsSlice, HasLen, Named, SerdeAny,
};
use serde::{Deserialize, Serialize};

use crate::fuzzer::context::{Context, MutationMetadata};

pub const IJON_MAP_SIZE: usize = 65_536;
pub const IJON_MAX_ENTRIES: usize = 512;
pub const IJON_MAX_BYTES: usize = IJON_MAX_ENTRIES * std::mem::size_of::<u64>();
pub const IJON_DEFAULT_SCHEDULE_PERCENT: usize = 80;
pub const IJON_HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, SerdeAny)]
pub struct IjonMaxMinMetadata {
    max_values: Vec<u64>,
    slot_inputs: Vec<Option<CorpusId>>,
    retire_max: bool,
}

impl IjonMaxMinMetadata {
    pub fn new(retire_max: bool) -> Self {
        Self {
            max_values: vec![0; IJON_MAX_ENTRIES],
            slot_inputs: vec![None; IJON_MAX_ENTRIES],
            retire_max,
        }
    }

    fn set_retire_max(&mut self, retire_max: bool) {
        self.retire_max = retire_max;
    }

    fn is_retired(&self, slot: usize) -> bool {
        self.retire_max && self.max_values[slot] == u64::MAX
    }

    fn retire_slot(&mut self, slot: usize) {
        self.max_values[slot] = u64::MAX;
        self.slot_inputs[slot] = None;
    }

    fn update_slot(&mut self, slot: usize, value: u64) {
        self.max_values[slot] = value;
    }

    fn value(&self, slot: usize) -> u64 {
        self.max_values[slot]
    }

    pub fn set_slot_input(&mut self, slot: usize, id: CorpusId) {
        if slot < self.slot_inputs.len() && !self.is_retired(slot) {
            self.slot_inputs[slot] = Some(id);
        }
    }

    pub fn live_slot_inputs<I, C>(&self, corpus: &C) -> Vec<CorpusId>
    where
        C: Corpus<I>,
    {
        self.slot_inputs
            .iter()
            .enumerate()
            .filter_map(|(slot, id)| {
                let id = (*id)?;
                if self.max_values[slot] == 0 || self.is_retired(slot) || corpus.get(id).is_err() {
                    None
                } else {
                    Some(id)
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SerdeAny)]
pub struct IjonMaxMinSlotsMetadata {
    slots: Vec<usize>,
}

impl IjonMaxMinSlotsMetadata {
    fn new(slots: Vec<usize>) -> Self {
        Self { slots }
    }
}

#[derive(Debug)]
pub struct IjonMaxMinFeedback<I, TC, C, O> {
    enabled: bool,
    retire_max: bool,
    history_limit: Option<usize>,
    history_index: usize,
    history_seen_slots: Vec<bool>,
    history_seen_count: usize,
    observer_handle: Handle<C>,
    bytes_converter: TC,
    structured_dir: PathBuf,
    rendered_dir: PathBuf,
    history_dir: PathBuf,
    pending_slots: Vec<usize>,
    name: Cow<'static, str>,
    phantom: PhantomData<(I, O)>,
}

impl<I, TC, C, O> IjonMaxMinFeedback<I, TC, C, O>
where
    C: AsRef<O> + Handled,
{
    pub fn new(enabled: bool, ijon_observer: &C, bytes_converter: TC, out_dir: PathBuf) -> Self {
        let history_limit = std::env::var("AFL_IJON_HISTORY_LIMIT")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|limit| *limit > 0)
            .map(|limit| limit.min(IJON_HISTORY_LIMIT))
            .or(Some(IJON_HISTORY_LIMIT));
        Self {
            enabled,
            retire_max: std::env::var_os("AFL_IJON_RETIRE_MAX").is_some(),
            history_limit,
            history_index: 0,
            history_seen_slots: vec![false; IJON_MAX_ENTRIES],
            history_seen_count: 0,
            observer_handle: ijon_observer.handle(),
            bytes_converter,
            structured_dir: out_dir.join("structured"),
            rendered_dir: out_dir.join("rendered"),
            history_dir: out_dir.join("history"),
            pending_slots: vec![],
            name: Cow::Borrowed("IjonMaxMinFeedback"),
            phantom: PhantomData,
        }
    }

    fn write_slot_files(&mut self, slot: usize, input: &I, rendered: &[u8]) -> Result<(), Error>
    where
        I: Input,
    {
        input.to_file(self.structured_dir.join(slot.to_string()))?;
        write_bytes_atomic(&self.rendered_dir.join(slot.to_string()), rendered)?;
        self.write_history_file(slot, rendered)?;
        Ok(())
    }

    fn write_history_file(&mut self, slot: usize, rendered: &[u8]) -> Result<(), Error> {
        let Some(limit) = self.history_limit else {
            return Ok(());
        };

        if !self.history_seen_slots[slot] {
            if self.history_seen_count + 1 > limit {
                return Err(Error::illegal_state(format!(
                    "AFL_IJON_HISTORY_LIMIT={limit} is too small for {} IJON slots",
                    self.history_seen_count + 1
                )));
            }
            self.history_seen_slots[slot] = true;
            self.history_seen_count += 1;
        }

        let index = self.history_index % limit;
        self.history_index = self.history_index.wrapping_add(1);
        let padding = (limit.saturating_sub(1).to_string().len()).max(3);
        let filename = format!("finding_{index:0padding$}.dat");
        write_bytes_atomic(&self.history_dir.join(filename), rendered)
    }
}

impl<I, TC, C, O> Named for IjonMaxMinFeedback<I, TC, C, O> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<I, TC, C, O, S> StateInitializer<S> for IjonMaxMinFeedback<I, TC, C, O>
where
    S: HasMetadata,
{
    fn init_state(&mut self, state: &mut S) -> Result<(), Error> {
        if self.enabled && !state.has_metadata::<IjonMaxMinMetadata>() {
            state.add_metadata(IjonMaxMinMetadata::new(self.retire_max));
        }
        Ok(())
    }
}

impl<I, TC, C, O, EM, OT, S> Feedback<EM, I, OT, S> for IjonMaxMinFeedback<I, TC, C, O>
where
    I: Input,
    TC: ToTargetBytes<I> + Clone,
    C: AsRef<O> + Handled,
    O: MapObserver<Entry = u8>,
    OT: MatchName,
    S: HasMetadata,
{
    fn is_interesting(
        &mut self,
        state: &mut S,
        _manager: &mut EM,
        _input: &I,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error> {
        self.pending_slots.clear();
        if !self.enabled {
            return Ok(false);
        }

        let observer = observers
            .get(&self.observer_handle)
            .ok_or_else(|| Error::key_not_found("IJON observer not found".to_string()))?
            .as_ref();
        if observer.len() < IJON_MAX_BYTES {
            return Err(Error::illegal_state(format!(
                "IJON observer too small: got {} bytes, need {IJON_MAX_BYTES}",
                observer.len()
            )));
        }

        let map = observer.to_vec();
        let metadata = state.metadata_or_insert_with(|| IjonMaxMinMetadata::new(self.retire_max));
        metadata.set_retire_max(self.retire_max);

        for (slot, chunk) in map.chunks_exact(8).take(IJON_MAX_ENTRIES).enumerate() {
            let value = u64::from_ne_bytes(chunk.try_into().expect("chunk length is 8"));

            if metadata.is_retired(slot) {
                continue;
            }

            if self.retire_max && value == u64::MAX {
                metadata.retire_slot(slot);
                continue;
            }

            if value > metadata.value(slot) {
                metadata.update_slot(slot, value);
                self.pending_slots.push(slot);
            }
        }

        Ok(!self.pending_slots.is_empty())
    }

    fn append_metadata(
        &mut self,
        state: &mut S,
        _manager: &mut EM,
        _observers: &OT,
        testcase: &mut Testcase<I>,
    ) -> Result<(), Error> {
        if self.pending_slots.is_empty() {
            return Ok(());
        }

        testcase.add_metadata(IjonMaxMinSlotsMetadata::new(self.pending_slots.clone()));
        if let Ok(context) = state.metadata_mut::<Context>() {
            context.add_mutation(MutationMetadata::Ijon);
        }

        let input = testcase
            .input()
            .as_ref()
            .ok_or_else(|| Error::illegal_state("IJON testcase has no input".to_string()))?;
        let rendered = self.bytes_converter.to_target_bytes(input);
        let rendered = rendered.as_slice();
        let pending_slots = std::mem::take(&mut self.pending_slots);
        for slot in pending_slots {
            self.write_slot_files(slot, input, rendered)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct IjonMaxMinScheduler<S> {
    inner: S,
    enabled: bool,
    schedule_percent: usize,
}

impl<S> IjonMaxMinScheduler<S> {
    pub fn new(inner: S, enabled: bool, schedule_percent: usize) -> Self {
        Self {
            inner,
            enabled,
            schedule_percent,
        }
    }
}

impl<CS, I, S> Scheduler<I, S> for IjonMaxMinScheduler<CS>
where
    CS: Scheduler<I, S>,
    S: HasCorpus<I> + HasMetadata + HasRand,
{
    fn on_add(&mut self, state: &mut S, id: CorpusId) -> Result<(), Error> {
        self.inner.on_add(state, id)?;
        if !self.enabled {
            return Ok(());
        }

        let slots = {
            let testcase = state.corpus().get(id)?.borrow();
            testcase
                .metadata::<IjonMaxMinSlotsMetadata>()
                .ok()
                .map(|metadata| metadata.slots.clone())
        };

        if let Some(slots) = slots {
            let metadata = state.metadata_or_insert_with(|| IjonMaxMinMetadata::new(false));
            for slot in slots {
                metadata.set_slot_input(slot, id);
            }
        }

        Ok(())
    }

    fn on_evaluation<OT>(&mut self, state: &mut S, input: &I, observers: &OT) -> Result<(), Error>
    where
        OT: MatchName,
    {
        self.inner.on_evaluation(state, input, observers)
    }

    fn next(&mut self, state: &mut S) -> Result<CorpusId, Error> {
        if self.enabled && self.schedule_percent > 0 && state.has_metadata::<IjonMaxMinMetadata>() {
            let choose_ijon = state.rand_mut().below(NonZero::new(100).expect("non-zero"))
                < self.schedule_percent;
            if choose_ijon {
                let live_inputs = state
                    .metadata::<IjonMaxMinMetadata>()?
                    .live_slot_inputs(state.corpus());
                if !live_inputs.is_empty() {
                    let idx = state
                        .rand_mut()
                        .below(NonZero::new(live_inputs.len()).expect("non-zero"));
                    let id = live_inputs[idx];
                    self.set_current_scheduled(state, Some(id))?;
                    return Ok(id);
                }
            }
        }

        self.inner.next(state)
    }

    fn set_current_scheduled(
        &mut self,
        state: &mut S,
        next_id: Option<CorpusId>,
    ) -> Result<(), Error> {
        self.inner.set_current_scheduled(state, next_id)
    }
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ijon_constants_match_afl_layout() {
        assert_eq!(IJON_MAX_ENTRIES, 512);
        assert_eq!(IJON_MAP_SIZE, 65_536);
        assert_eq!(IJON_MAX_BYTES, 4_096);
        assert_eq!(IJON_HISTORY_LIMIT, 100);
    }

    #[test]
    fn metadata_tracks_values_and_retirement() {
        let mut metadata = IjonMaxMinMetadata::new(true);
        metadata.update_slot(0, 7);
        metadata.set_slot_input(0, CorpusId(0));
        metadata.retire_slot(1);
        assert_eq!(metadata.max_values[0], 7);
        assert!(metadata.is_retired(1));
    }
}
