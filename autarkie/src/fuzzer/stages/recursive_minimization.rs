use crate::{MutationType, Node, NodeType, Visitor};
use libafl::{
    corpus::Corpus,
    events::EventFirer,
    executors::{Executor, HasObservers},
    feedbacks::{HasObserverHandle, MapIndexesMetadata},
    observers::{CanTrack, MapObserver, ObserversTuple},
    stages::{Restartable, Stage},
    state::{HasCorpus, HasCurrentTestcase},
    Evaluator, HasMetadata,
};
use libafl_bolts::{tuples::Handle, AsIter, Named};
use num_traits::Bounded;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::VecDeque,
    fmt::Debug,
    marker::PhantomData,
    rc::Rc,
};

use crate::fuzzer::context::Context;

use super::stats::AutarkieStats;

#[derive(Debug)]
pub struct RecursiveMinimizationStage<C, E, O, OT, S, I> {
    map_observer_handle: Handle<C>,
    map_name: Cow<'static, str>,
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<(E, O, OT, S, I)>,
}

impl<C, E, O, OT, S, I> RecursiveMinimizationStage<C, E, O, OT, S, I>
where
    O: MapObserver,
    for<'it> O: AsIter<'it, Item = O::Entry>,
    C: AsRef<O> + CanTrack,
    OT: ObserversTuple<I, S>,
{
    pub fn new<F>(visitor: Rc<RefCell<Visitor>>, map_feedback: &F) -> Self
    where
        F: HasObserverHandle<Observer = C> + Named,
    {
        let map_name = map_feedback.name().clone();
        Self {
            map_observer_handle: map_feedback.observer_handle().clone(),
            map_name: map_name.clone(),
            visitor,
            phantom: PhantomData,
        }
    }
}

impl<C, E, O, OT, S, I, EM, Z> Stage<E, EM, S, Z> for RecursiveMinimizationStage<C, E, O, OT, S, I>
where
    I: Node + Serialize + Clone,
    S: HasCurrentTestcase<I> + HasCorpus<I> + HasMetadata,
    E: Executor<EM, I, S, Z> + HasObservers<Observers = OT>,
    Z: Evaluator<E, EM, I, S>,
    EM: EventFirer<I, S>,
    O: MapObserver,
    C: AsRef<O> + CanTrack,
    for<'de> <O as MapObserver>::Entry:
        Serialize + Deserialize<'de> + 'static + Default + Debug + Bounded,
    OT: ObserversTuple<I, S>,
{
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        executor: &mut E,
        state: &mut S,
        manager: &mut EM,
    ) -> Result<(), libafl::Error> {
        let metadata = state.metadata::<Context>().unwrap();
        let indexes = state
            .current_testcase()
            .unwrap()
            .borrow()
            .metadata::<MapIndexesMetadata>()
            .unwrap()
            .list
            .clone();

        let mut current = state.current_input_cloned().unwrap();
        current.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        let mut fields = self.visitor.borrow_mut().fields();

        let mut skip = 0;
        let mut found = false;
        let mut cur_iter = 0;

        loop {
            let field = fields.pop();
            if field.is_none() {
                break;
            }
            let field = field.unwrap();
            let ((id, node_ty), ty) = field.last().unwrap();
            if let NodeType::Recursive = node_ty {
                if cur_iter < skip {
                    cur_iter += 1;
                    continue;
                }
                let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                let mut inner = current.clone();
                // We are only trying to replace with one non recursive variant (maybe try to replace with ALL possible non recursive varaints?)
                inner.__autarkie_mutate(
                    &mut MutationType::RecursiveReplace,
                    &mut self.visitor.borrow_mut(),
                    path.clone(),
                );
                let run = fuzzer.evaluate_input(state, executor, manager, &inner)?;
                let map = &executor.observers()[&self.map_observer_handle]
                    .as_ref()
                    .to_vec();
                let map = map
                    .into_iter()
                    .enumerate()
                    .filter(|i| i.1 != &O::Entry::default())
                    .map(|i| i.0)
                    .collect::<Vec<_>>();
                if map == indexes {
                    found = true;
                    cur_iter = 0;
                    current = inner;
                    current.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
                    fields = self.visitor.borrow_mut().fields();
                } else {
                    skip += 1;
                }
                cur_iter += 1;
            }
        }
        if found {
            let metadata = state
                .metadata_mut::<AutarkieStats>()
                .unwrap()
                .add_new_input_mutation(
                    crate::fuzzer::context::MutationMetadata::RecursiveMinimization,
                );
        }
        state.current_testcase_mut()?.set_input(current);
        Ok(())
    }
}

impl<C, E, O, OT, S, I> Restartable<S> for RecursiveMinimizationStage<C, E, O, OT, S, I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl::Error> {
        Ok(())
    }
}
