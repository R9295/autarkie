use core::marker::PhantomData;
use crate::fuzzer::context::{MutationMetadata, Context};
use crate::{Node, Visitor};
use std::cell::RefCell;
use std::rc::Rc;
use crate::MutationType;
use std::collections::VecDeque;
use std::borrow::{Cow, ToOwned};
use std::collections::HashSet;
use libafl::{
    Evaluator,
    corpus::HasCurrentCorpusId,
    executors::{Executor, HasObservers},
    observers::{CmpValues, ObserversTuple, AflppCmpValuesMetadata},
    stages::{colorization::TaintMetadata, Restartable, RetryCountRestartHelper, Stage},
    state::{HasCorpus, HasCurrentTestcase},
    Error, HasMetadata, HasNamedMetadata,
};
use libafl_bolts::{
    tuples::{Handle, MatchNameRef},
    Named,
};

use libafl_targets::AflppCmpLogObserver;

/// Trace with tainted input
#[derive(Debug, Clone)]
pub struct CmpLogStage<'a, EM, TE, S, Z, I> {
    visitor: Rc<RefCell<Visitor>>,
    name: Cow<'static, str>,
    tracer_executor: TE,
    cmplog_observer_handle: Handle<AflppCmpLogObserver<'a>>,
    phantom: PhantomData<(EM, TE, S, Z, I)>,
}
/// The name for aflpp tracing stage
pub static AFLPP_CMPLOG_TRACING_STAGE_NAME: &str = "aflpptracing";

impl<EM, TE, S, Z, I> Named for CmpLogStage<'_, EM, TE, S, Z, I> {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<E, EM, TE, S, Z, I> Stage<E, EM, S, Z> for CmpLogStage<'_, EM, TE, S, Z, I>
where
    I: Node + Clone,
    TE: HasObservers + Executor<EM, I, S, Z>,
    TE::Observers: MatchNameRef + ObserversTuple<I, S>,
    S: HasCorpus<I> + HasCurrentTestcase<I> + HasMetadata + HasNamedMetadata + HasCurrentCorpusId,
    Z: Evaluator<E, EM, I, S>,
{
    #[inline]
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), Error> {
        // First run with the un-mutated input
        let mut unmutated_input = state.current_input_cloned()?;

        if let Some(ob) = self
            .tracer_executor
            .observers_mut()
            .get_mut(&self.cmplog_observer_handle)
        {
            // This is not the original input,
            // Set it to false
            ob.set_original(true);
        }
        // I can't think of any use of this stage if you don't use AflppCmpLogObserver
        // but do nothing ofcourse

        self.tracer_executor
            .observers_mut()
            .pre_exec_all(state, &unmutated_input)?;

        let exit_kind =
            self.tracer_executor
                .run_target(fuzzer, state, manager, &unmutated_input)?;

        self.tracer_executor
            .observers_mut()
            .post_exec_all(state, &unmutated_input, &exit_kind)?;
        let mut reduced = HashSet::new();
        if let Ok(data) = state.metadata::<AflppCmpValuesMetadata>() {
            for item in data.orig_cmpvals().values() {
                    for i in item.into_iter() {
                if let Some((left, right, _is_const)) = i.to_u64_tuple() {
                    reduced.insert((left, right));
                    }
                }
            }
        }
          for cmp in reduced {
            unmutated_input.__autarkie_cmps(&mut self.visitor.borrow_mut(), 0, cmp);
            let matches = self.visitor.borrow_mut().cmps();
            for path in matches {
                let cmp_path = path.0.iter().map(|(i, ty)| i.0).collect::<VecDeque<_>>();
                let mut serialized_alternative = path.1.as_slice();
                state.metadata_mut::<Context>().unwrap().add_mutation(MutationMetadata::Cmplog);
                #[cfg(debug_assertions)]
                println!("cmplog_splice | one | {:?}", path.0);
                unmutated_input.__autarkie_mutate(
                    &mut MutationType::Splice(&mut serialized_alternative),
                    &mut self.visitor.borrow_mut(),
                    cmp_path,
                );
                let res = fuzzer.evaluate_input(state, executor, manager, &unmutated_input)?;
                    if res.0.is_corpus() {
                        println!("{:?}", res);
                }
            }
        }

        Ok(())
    }
}

impl<EM, TE, S, Z, I> Restartable<S> for CmpLogStage<'_, EM, TE, S, Z, I>
where
    S: HasMetadata + HasNamedMetadata + HasCurrentCorpusId,
{
    fn should_restart(&mut self, state: &mut S) -> Result<bool, Error> {
        // Tracing stage is always deterministic
        // don't restart
        RetryCountRestartHelper::no_retry(state, &self.name)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), Error> {
        // TODO: this may need better resumption? (Or is it always used with a forkserver?)
        RetryCountRestartHelper::clear_progress(state, &self.name)
    }
}

impl<'a, EM, TE, S, Z, I> CmpLogStage<'a, EM, TE, S, Z, I> {
    /// With cmplog observer
    pub fn new(visitor: Rc<RefCell<Visitor>>, tracer_executor: TE, observer_handle: Handle<AflppCmpLogObserver<'a>>) -> Self {
        let observer_name = observer_handle.name().clone();
        Self {
            visitor,
            name: Cow::Owned(
                AFLPP_CMPLOG_TRACING_STAGE_NAME.to_owned()
                    + ":"
                    + observer_name.into_owned().as_str(),
            ),
            cmplog_observer_handle: observer_handle,
            tracer_executor,
            phantom: PhantomData,
        }
    }

    /// Gets the underlying tracer executor
    pub fn executor(&self) -> &TE {
        &self.tracer_executor
    }

    /// Gets the underlying tracer executor (mut)
    pub fn executor_mut(&mut self) -> &mut TE {
        &mut self.tracer_executor
    }
}
