#![allow(warnings)]
use std::{borrow::Cow, collections::HashSet, fmt::Debug, marker::PhantomData};

use libafl::{
    corpus::{Corpus, HasCurrentCorpusId},
    events::{Event, EventFirer, EventWithStats},
    executors::{Executor, ExitKind, HasObservers},
    feedbacks::{map::MapFeedbackMetadata, HasObserverHandle},
    monitors::stats::{AggregatorOps, UserStats, UserStatsValue},
    observers::{MapObserver, ObserversTuple},
    stages::{run_target_with_timing, Restartable, RetryCountRestartHelper, Stage},
    state::{HasCorpus, HasCurrentTestcase, HasExecutions},
    Error, HasMetadata, HasNamedMetadata,
};
use libafl_bolts::{tuples::Handle, AsIter, Named};
use num_traits::Bounded;
use serde::{Deserialize, Serialize};

const STABILITY_STAGE_RUNS: usize = 3;

#[derive(Serialize, Deserialize, Debug, Clone, Default, libafl_bolts::SerdeAny)]
pub struct StabilityMetadata {
    variable_entries: HashSet<usize>,
    discovered_entries: usize,
}

#[derive(Debug, Clone)]
pub struct StabilityStage<C, I, O, OT, S> {
    map_observer_handle: Handle<C>,
    map_name: Cow<'static, str>,
    name: Cow<'static, str>,
    phantom: PhantomData<(I, O, OT, S)>,
}

impl<C, I, O, OT, S> StabilityStage<C, I, O, OT, S>
where
    C: AsRef<O>,
    O: MapObserver,
    for<'it> O: AsIter<'it, Item = O::Entry>,
    OT: ObserversTuple<I, S>,
{
    pub fn new<F>(map_feedback: &F) -> Self
    where
        F: HasObserverHandle<Observer = C> + Named,
    {
        let map_name = map_feedback.name().clone();
        Self {
            map_observer_handle: map_feedback.observer_handle().clone(),
            map_name: map_name.clone(),
            name: Cow::Owned(format!("stability:{}", map_name.into_owned())),
            phantom: PhantomData,
        }
    }
}

impl<C, E, EM, I, O, OT, S, Z> Stage<E, EM, S, Z> for StabilityStage<C, I, O, OT, S>
where
    E: Executor<EM, I, S, Z> + HasObservers<Observers = OT>,
    EM: EventFirer<I, S>,
    O: MapObserver,
    C: AsRef<O>,
    for<'de> <O as MapObserver>::Entry:
        Serialize + Deserialize<'de> + 'static + Default + Debug + Bounded,
    OT: ObserversTuple<I, S>,
    S: HasCorpus<I>
        + HasMetadata
        + HasNamedMetadata
        + HasExecutions
        + HasCurrentTestcase<I>
        + HasCurrentCorpusId,
    I: libafl::inputs::Input,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        mgr: &mut EM,
    ) -> Result<(), Error> {
        // Only measure stability once per corpus entry, the first time it is scheduled.
        {
            let testcase = state.current_testcase()?;
            if testcase.scheduled_count() > 0 {
                return Ok(());
            }
        }

        let input = state.current_input_cloned()?;
        run_target_with_timing(fuzzer, executor, state, mgr, &input, false)?;

        let observers = &executor.observers();
        let map_first = observers[&self.map_observer_handle].as_ref();
        let discovered = match state
            .named_metadata_map()
            .get::<MapFeedbackMetadata<O::Entry>>(&self.map_name)
        {
            Some(metadata) => metadata.num_covered_map_indexes,
            None => map_first.count_bytes() as usize,
        };
        let map_first_entries = map_first.to_vec();
        let map_first_len = map_first_entries.len();
        let mut variable_entries: Vec<usize> = vec![];

        let mut i = 1;
        let mut has_errors = false;
        while i < STABILITY_STAGE_RUNS {
            let (exit_kind, _duration, has_errors_result) =
                run_target_with_timing(fuzzer, executor, state, mgr, &input, has_errors)?;
            has_errors = has_errors_result;

            if exit_kind != ExitKind::Timeout {
                let map = executor.observers()[&self.map_observer_handle]
                    .as_ref()
                    .to_vec();
                let map_state = state
                    .named_metadata_map_mut()
                    .get_mut::<MapFeedbackMetadata<O::Entry>>(&self.map_name)
                    .unwrap();
                let history_map = &mut map_state.history_map;
                if history_map.len() < map_first_len {
                    history_map.resize(map_first_len, O::Entry::default());
                }
                for (idx, (first, (cur, history))) in map_first_entries
                    .iter()
                    .zip(map.iter().zip(history_map.iter_mut()))
                    .enumerate()
                {
                    if *first != *cur && *history != O::Entry::max_value() {
                        map_state.num_covered_map_indexes +=
                            usize::from(*history == O::Entry::default());
                        *history = O::Entry::max_value();
                        variable_entries.push(idx);
                    }
                }
            }
            i += 1;
        }

        let discovered = match state
            .named_metadata_map()
            .get::<MapFeedbackMetadata<O::Entry>>(&self.map_name)
        {
            Some(metadata) => metadata.num_covered_map_indexes,
            None => discovered,
        };

        {
            let meta = state.metadata_or_insert_with(StabilityMetadata::default);
            for entry in variable_entries {
                meta.variable_entries.insert(entry);
            }
            meta.discovered_entries = discovered;
        }

        if let Some(meta) = state.metadata_map().get::<StabilityMetadata>() {
            let variable = meta.variable_entries.len();
            let discovered = meta.discovered_entries;
            if discovered > 0 {
                let stable = (discovered.saturating_sub(variable)) as u64;
                mgr.fire(
                    state,
                    EventWithStats::with_current_time(
                        Event::UpdateUserStats {
                            name: Cow::from("stability"),
                            value: UserStats::new(
                                UserStatsValue::Ratio(stable, discovered as u64),
                                AggregatorOps::Avg,
                            ),
                            phantom: PhantomData,
                        },
                        *state.executions(),
                    ),
                )?;
            }
        }

        Ok(())
    }
}

impl<C, I, O, OT, S> Restartable<S> for StabilityStage<C, I, O, OT, S>
where
    S: HasMetadata + HasNamedMetadata + HasCurrentCorpusId,
{
    fn should_restart(&mut self, state: &mut S) -> Result<bool, Error> {
        RetryCountRestartHelper::no_retry(state, &self.name)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), Error> {
        RetryCountRestartHelper::clear_progress(state, &self.name)
    }
}

impl<C, I, O, OT, S> Named for StabilityStage<C, I, O, OT, S> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}
