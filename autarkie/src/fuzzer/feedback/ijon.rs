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

    use libafl::{
        corpus::InMemoryCorpus,
        inputs::{BytesInput, HasTargetBytes},
        observers::StdMapObserver,
        state::NopState,
    };
    use libafl_bolts::{ownedref::OwnedSlice, tuples::tuple_list};

    /// Minimal [`ToTargetBytes`] used to render IJON findings to disk in the
    /// feedback tests; the structured `BytesInput` is its own target encoding.
    #[derive(Clone)]
    struct TargetBytesConverter;

    impl ToTargetBytes<BytesInput> for TargetBytesConverter {
        fn to_target_bytes<'a>(&mut self, input: &'a BytesInput) -> OwnedSlice<'a, u8> {
            input.target_bytes()
        }
    }

    /// Write a `u64` into IJON slot `slot` of the shared map exactly the way
    /// AFL++'s `ijon_max()` writes `__afl_ijon_bits[var_id]` (native-endian,
    /// 8 bytes per entry). Mirrors how the forkserver target updates the map.
    fn set_slot_value(observer: &mut StdMapObserver<'_, u8, false>, slot: usize, value: u64) {
        let map: &mut [u8] = observer;
        let start = slot * core::mem::size_of::<u64>();
        map[start..start + core::mem::size_of::<u64>()].copy_from_slice(&value.to_ne_bytes());
    }

    /// AFL++ implements `IJON_MIN(x)` as `ijon_max(addr, u64::MAX - x)`
    /// (see `ijon_min` in `instrumentation/afl-compiler-rt.o.c`). So a smaller
    /// `x` is encoded as a *larger* stored slot value. This helper reproduces
    /// that target-side encoding so the test drives the exact bytes the real
    /// runtime would deposit for a minimization goal.
    fn ijon_min_encode(x: u64) -> u64 {
        u64::MAX - x
    }

    fn new_observer() -> StdMapObserver<'static, u8, false> {
        StdMapObserver::owned("ijon_max_min", vec![0u8; IJON_MAX_BYTES])
    }

    fn is_interesting<TC, C, O>(
        feedback: &mut IjonMaxMinFeedback<BytesInput, TC, C, O>,
        state: &mut NopState<BytesInput>,
        observers: &impl MatchName,
        input: &BytesInput,
    ) -> bool
    where
        TC: ToTargetBytes<BytesInput> + Clone,
        C: AsRef<O> + Handled,
        O: MapObserver<Entry = u8>,
    {
        feedback
            .is_interesting(state, &mut (), input, observers, &ExitKind::Ok)
            .expect("is_interesting failed")
    }

    #[test]
    fn ijon_constants_match_afl_layout() {
        // Must match AFL++'s `config.h`: MAP_SIZE_IJON_ENTRIES / MAP_SIZE_IJON_MAP
        // and MAP_SIZE_IJON_BYTES (= entries * sizeof(u64)).
        assert_eq!(IJON_MAX_ENTRIES, 512);
        assert_eq!(IJON_MAP_SIZE, 65_536);
        assert_eq!(IJON_MAX_BYTES, 4_096);
        assert_eq!(IJON_MAX_BYTES, IJON_MAX_ENTRIES * core::mem::size_of::<u64>());
        assert_eq!(IJON_HISTORY_LIMIT, 100);
        assert_eq!(IJON_DEFAULT_SCHEDULE_PERCENT, 80);
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

    #[test]
    fn metadata_retirement_only_when_enabled() {
        // With retire_max disabled, a slot saturated at u64::MAX is NOT retired:
        // it is just a very large maximum that can never be improved.
        let mut keep = IjonMaxMinMetadata::new(false);
        keep.update_slot(3, u64::MAX);
        assert!(!keep.is_retired(3));

        // With retire_max enabled, saturating a slot at u64::MAX retires it and
        // forgets its scheduled input (the optimization goal has been reached).
        let mut retire = IjonMaxMinMetadata::new(true);
        retire.update_slot(3, 5);
        retire.set_slot_input(3, CorpusId(1));
        assert!(!retire.is_retired(3));
        retire.update_slot(3, u64::MAX);
        assert!(retire.is_retired(3));
        // set_slot_input is a no-op for retired slots.
        retire.set_slot_input(3, CorpusId(2));
        assert_eq!(retire.slot_inputs[3], Some(CorpusId(1)));
    }

    /// IJON_MAX: the feedback flags an input interesting whenever any slot
    /// reaches a new maximum, and tracks the running max per slot.
    #[test]
    fn feedback_tracks_ijon_max() {
        let out = tempdir::TempDir::new("ijon_max").expect("tempdir");
        let observer = new_observer();
        let mut feedback: IjonMaxMinFeedback<BytesInput, _, _, _> = IjonMaxMinFeedback::new(
            true,
            &observer,
            TargetBytesConverter,
            out.path().to_path_buf(),
        );
        let mut observers = tuple_list!(observer);
        let mut state = NopState::<BytesInput>::new();
        feedback.init_state(&mut state).expect("init_state");
        let input = BytesInput::from(&b"x"[..]);

        // First non-zero observation on slot 7 is a new max -> interesting.
        set_slot_value(&mut observers.0, 7, 10);
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));
        assert_eq!(feedback.pending_slots, vec![7]);

        // Same value again: not a new max -> not interesting.
        assert!(!is_interesting(&mut feedback, &mut state, &observers, &input));
        assert!(feedback.pending_slots.is_empty());

        // A larger value -> interesting again.
        set_slot_value(&mut observers.0, 7, 25);
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));
        assert_eq!(feedback.pending_slots, vec![7]);

        // A smaller value -> not interesting (max-only semantics).
        set_slot_value(&mut observers.0, 7, 5);
        assert!(!is_interesting(&mut feedback, &mut state, &observers, &input));

        // The metadata holds the running maximum, not the latest observation.
        assert_eq!(state.metadata::<IjonMaxMinMetadata>().unwrap().value(7), 25);
    }

    /// IJON_MIN: minimization is target-side `u64::MAX - x`, so a *decreasing*
    /// quantity is observed by the feedback as an *increasing* slot value. The
    /// same max machinery therefore drives minimization correctly.
    #[test]
    fn feedback_tracks_ijon_min() {
        let out = tempdir::TempDir::new("ijon_min").expect("tempdir");
        let observer = new_observer();
        let mut feedback: IjonMaxMinFeedback<BytesInput, _, _, _> = IjonMaxMinFeedback::new(
            true,
            &observer,
            TargetBytesConverter,
            out.path().to_path_buf(),
        );
        let mut observers = tuple_list!(observer);
        let mut state = NopState::<BytesInput>::new();
        feedback.init_state(&mut state).expect("init_state");
        let input = BytesInput::from(&b"y"[..]);

        // Minimization goal on slot 3: x = 100 (far from the 0 optimum).
        set_slot_value(&mut observers.0, 3, ijon_min_encode(100));
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));
        assert_eq!(feedback.pending_slots, vec![3]);

        // x improves to 50 -> encoded value rises -> interesting.
        set_slot_value(&mut observers.0, 3, ijon_min_encode(50));
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));

        // x improves further to 10 -> still interesting.
        set_slot_value(&mut observers.0, 3, ijon_min_encode(10));
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));

        // x regresses to 20 (worse than 10) -> encoded value drops -> boring.
        set_slot_value(&mut observers.0, 3, ijon_min_encode(20));
        assert!(!is_interesting(&mut feedback, &mut state, &observers, &input));

        // The retained maximum corresponds to the best (smallest) x seen: 10.
        assert_eq!(
            state.metadata::<IjonMaxMinMetadata>().unwrap().value(3),
            ijon_min_encode(10)
        );
    }

    /// With AFL_IJON_RETIRE_MAX set, a slot that saturates at u64::MAX (e.g. an
    /// IJON_MIN goal reaching its optimum x = 0) is retired: the saturating
    /// observation is not reported as interesting, and the slot is ignored from
    /// then on while other slots keep making progress.
    #[test]
    fn feedback_retires_saturated_slot() {
        // This is the only test that reads AFL_IJON_RETIRE_MAX; all other
        // feedback tests stay strictly below u64::MAX so they are unaffected
        // by the value of this process-global variable.
        std::env::set_var("AFL_IJON_RETIRE_MAX", "1");

        let out = tempdir::TempDir::new("ijon_retire").expect("tempdir");
        let observer = new_observer();
        let mut feedback: IjonMaxMinFeedback<BytesInput, _, _, _> = IjonMaxMinFeedback::new(
            true,
            &observer,
            TargetBytesConverter,
            out.path().to_path_buf(),
        );
        assert!(feedback.retire_max, "AFL_IJON_RETIRE_MAX must enable retirement");
        let mut observers = tuple_list!(observer);
        let mut state = NopState::<BytesInput>::new();
        feedback.init_state(&mut state).expect("init_state");
        let input = BytesInput::from(&b"z"[..]);

        // Slot 9 climbs normally toward its minimization optimum.
        set_slot_value(&mut observers.0, 9, ijon_min_encode(100));
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));

        // Goal reached: x = 0 -> encoded u64::MAX -> slot is retired, and the
        // saturating observation itself is NOT reported as a new max.
        set_slot_value(&mut observers.0, 9, u64::MAX);
        assert!(!is_interesting(&mut feedback, &mut state, &observers, &input));
        let md = state.metadata::<IjonMaxMinMetadata>().unwrap();
        assert!(md.is_retired(9));
        assert_eq!(md.value(9), u64::MAX);

        // Any later observation on a retired slot is ignored.
        set_slot_value(&mut observers.0, 9, ijon_min_encode(5));
        assert!(!is_interesting(&mut feedback, &mut state, &observers, &input));
        assert_eq!(state.metadata::<IjonMaxMinMetadata>().unwrap().value(9), u64::MAX);

        // A different, un-retired slot still registers progress.
        set_slot_value(&mut observers.0, 11, 42);
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));
        assert_eq!(feedback.pending_slots, vec![11]);

        std::env::remove_var("AFL_IJON_RETIRE_MAX");
    }

    /// On an interesting input the feedback persists the structured input, the
    /// rendered target bytes, and a history copy for each improved slot.
    #[test]
    fn feedback_writes_slot_files() {
        let out = tempdir::TempDir::new("ijon_files").expect("tempdir");
        for sub in ["structured", "rendered", "history"] {
            std::fs::create_dir_all(out.path().join(sub)).expect("mkdir");
        }
        let observer = new_observer();
        let mut feedback: IjonMaxMinFeedback<BytesInput, _, _, _> = IjonMaxMinFeedback::new(
            true,
            &observer,
            TargetBytesConverter,
            out.path().to_path_buf(),
        );
        let mut observers = tuple_list!(observer);
        let mut state = NopState::<BytesInput>::new();
        feedback.init_state(&mut state).expect("init_state");
        let input = BytesInput::from(&b"render-me"[..]);

        set_slot_value(&mut observers.0, 4, 123);
        assert!(is_interesting(&mut feedback, &mut state, &observers, &input));

        let mut testcase = Testcase::new(input);
        feedback
            .append_metadata(&mut state, &mut (), &observers, &mut testcase)
            .expect("append_metadata");

        // The improved slot (4) got a structured + rendered file.
        assert!(out.path().join("structured").join("4").exists());
        let rendered = std::fs::read(out.path().join("rendered").join("4")).expect("rendered file");
        assert_eq!(rendered, b"render-me");

        // Exactly one history entry was written for the single improved slot.
        let history: Vec<_> = std::fs::read_dir(out.path().join("history"))
            .expect("history dir")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(history.len(), 1);

        // The pending slots were consumed by append_metadata.
        assert!(feedback.pending_slots.is_empty());
    }

    /// The scheduler only ever schedules "live" slot inputs. live_slot_inputs is
    /// its selection core: it must drop zero-value slots, retired slots, and
    /// slots whose backing corpus entry no longer exists.
    #[test]
    fn live_slot_inputs_filters_unschedulable() {
        let mut corpus = InMemoryCorpus::<BytesInput>::new();
        let id_a = corpus.add(Testcase::new(BytesInput::from(&b"a"[..]))).unwrap();
        let id_b = corpus.add(Testcase::new(BytesInput::from(&b"b"[..]))).unwrap();
        let id_c = corpus.add(Testcase::new(BytesInput::from(&b"c"[..]))).unwrap();
        let id_removed = corpus.add(Testcase::new(BytesInput::from(&b"d"[..]))).unwrap();
        corpus.remove(id_removed).unwrap();

        let mut md = IjonMaxMinMetadata::new(true);

        // slot 0: value still 0 -> excluded (no progress recorded yet).
        md.set_slot_input(0, id_a);

        // slot 1: real max + live corpus entry -> included.
        md.update_slot(1, 5);
        md.set_slot_input(1, id_a);

        // slot 2: had an input, then saturated to u64::MAX -> retired -> excluded.
        md.update_slot(2, 3);
        md.set_slot_input(2, id_b);
        md.update_slot(2, u64::MAX);
        assert!(md.is_retired(2));

        // slot 3: input points at a corpus entry that was removed -> excluded.
        md.update_slot(3, 9);
        md.set_slot_input(3, id_removed);

        // slot 4: real max + live corpus entry -> included.
        md.update_slot(4, 3);
        md.set_slot_input(4, id_c);

        let live = md.live_slot_inputs(&corpus);
        assert_eq!(live.len(), 2);
        assert!(live.contains(&id_a));
        assert!(live.contains(&id_c));
        assert!(!live.contains(&id_b));
    }
}
