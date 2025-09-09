use crate::{
    fuzzer::context::{Context, MutationMetadata},
    Node, Visitor,
};
use libafl::{
    corpus::Corpus,
    events::{Event, EventFirer, EventWithStats},
    executors::Executor,
    monitors::stats::{UserStats, UserStatsValue},
    stages::{Restartable, Stage},
    state::{HasCorpus, HasCurrentTestcase, HasExecutions},
    Evaluator, HasMetadata,
};
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    fmt::format,
    marker::PhantomData,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct StatsStage<I> {
    last_run: Instant,
    out_dir: PathBuf,
    phantom: PhantomData<I>,
}

impl<I> StatsStage<I> {
    pub fn new(out_dir: PathBuf) -> Self {
        Self {
            last_run: Instant::now(),
            out_dir,
            phantom: PhantomData,
        }
    }
}

impl<E, EM, Z, S, I> Stage<E, EM, S, Z> for StatsStage<I>
where
    I: Node + Serialize,
    S: HasCurrentTestcase<I> + HasCorpus<I> + HasMetadata + HasExecutions,
    E: Executor<EM, I, S, Z>,
    EM: EventFirer<I, S>,
    Z: Evaluator<E, EM, I, S>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), libafl::Error> {
        if Instant::now() - self.last_run > Duration::from_secs(5) {
            let mut metadata = state.metadata::<AutarkieStats>()?.clone();
            let data = serde_json::to_string_pretty(&metadata).expect("____YR5BenN6");
            std::fs::write(self.out_dir.join("stats.json"), &data).expect("____weNooV3S");
            for (mutation, count) in metadata.mutations.iter() {
                manager.fire(
                    state,
                    EventWithStats::with_current_time(
                        Event::UpdateUserStats {
                            name: format!("{:?}", mutation).into(),
                            value: UserStats::new(
                                UserStatsValue::Number(*count as u64),
                                libafl::monitors::stats::AggregatorOps::Sum,
                            ),
                            phantom: PhantomData,
                        },
                        *state.executions(),
                    ),
                )?;
            }
            self.last_run = Instant::now();
        }
        Ok(())
    }
}

impl<I, S> Restartable<S> for StatsStage<I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl::Error> {
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, libafl_bolts::SerdeAny, Default)]
pub struct AutarkieStats {
    mutations: BTreeMap<MutationMetadata, usize>,
}

impl AutarkieStats {
    pub fn add_new_input_mutations(&mut self, mutations: HashSet<MutationMetadata>) {
        for m in mutations {
            self.mutations
                .entry(m)
                .and_modify(|v| {
                    *v += 1;
                })
                .or_insert(1);
        }
    }
    pub fn add_new_input_mutation(&mut self, m: MutationMetadata) {
        self.mutations
            .entry(m)
            .and_modify(|v| {
                *v += 1;
            })
            .or_insert(1);
    }
    pub fn mutations<'a>(&'a self) -> &BTreeMap<MutationMetadata, usize> {
        &self.mutations
    }
}
