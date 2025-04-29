use crate::{fuzzer::stages::stats::AutarkieStats, MutationType, Node, NodeType, Visitor};
use libafl::{
    corpus::Corpus,
    events::EventFirer,
    executors::{Executor, HasObservers},
    feedbacks::{HasObserverHandle, MapNoveltiesMetadata},
    observers::{CanTrack, MapObserver, ObserversTuple},
    stages::{Restartable, Stage},
    state::{HasCorpus, HasCurrentTestcase},
    Evaluator, HasMetadata,
};
use libafl_bolts::{tuples::Handle, AsIter, Named};
use num_traits::Bounded;
use serde::{Deserialize, Serialize};
use std::{
    borrow::{Borrow, Cow},
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    fmt::Debug,
    marker::PhantomData,
    rc::Rc,
};

use crate::fuzzer::Context;

#[derive(Debug)]
pub struct MinimizationStage<C, E, O, OT, S, I> {
    map_observer_handle: Handle<C>,
    map_name: Cow<'static, str>,
    visitor: Rc<RefCell<Visitor>>,
    phantom: PhantomData<(E, O, OT, S, I)>,
}

impl<C, E, O, OT, S, I> MinimizationStage<C, E, O, OT, S, I>
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

impl<C, E, O, OT, S, I, EM, Z> Stage<E, EM, S, Z> for MinimizationStage<C, E, O, OT, S, I>
where
    I: Node + Serialize + Clone,
    S: HasCurrentTestcase<I> + HasCorpus<I> + HasMetadata,
    E: Executor<EM, I, S, Z> + HasObservers<Observers = OT>,
    EM: EventFirer<I, S>,
    Z: Evaluator<E, EM, I, S>,
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
    ) -> Result<(), libafl_bolts::Error> {
        if state.current_testcase()?.scheduled_count() > 0 {
            return Ok(());
        }

        let metadata = state.metadata::<Context>().unwrap();
        let novelties = state
            .current_testcase()
            .unwrap()
            .borrow()
            .metadata::<MapNoveltiesMetadata>()
            .unwrap()
            .list
            .clone();
        let mut current = state.current_input_cloned().unwrap();
        current.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
        let mut skip = 0;
        let mut fields = self.visitor.borrow_mut().fields();
        let mut found = false;
        loop {
            let field = fields.pop();
            if field.is_none() {
                break;
            }
            let field = field.unwrap();
            let ((id, node_ty), ty) = field.last().unwrap();
            if let NodeType::Iterable(is_fixed_len, field_len, inner_ty) = node_ty {
                let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                // NOTE: -1 because we zero index
                let mut len = field_len.saturating_sub(1);
                let mut counter = 0;
                if *is_fixed_len {
                    continue;
                }
                loop {
                    if len == 0 || counter >= len {
                        break;
                    }
                    let mut inner = current.clone();
                    inner.__autarkie_mutate(
                        &mut MutationType::IterablePop(counter),
                        &mut self.visitor.borrow_mut(),
                        path.clone(),
                    );
                    let run = fuzzer.evaluate_input(state, executor, manager, &inner)?;
                    let map = &executor.observers()[&self.map_observer_handle]
                        .as_ref()
                        .how_many_set(&novelties);
                    if *map == novelties.len() {
                        found = true;
                        current = inner;
                        current.__autarkie_fields(&mut self.visitor.borrow_mut(), 0);
                        fields = self.visitor.borrow_mut().fields();
                        len = len.saturating_sub(1);
                    }
                    counter += 1;
                }
            }
        }
        state.current_testcase_mut()?.set_input(current.clone());
        if found {
            let metadata = state
                .metadata_mut::<AutarkieStats>()
                .unwrap()
                .add_new_input_mutation(
                    crate::fuzzer::context::MutationMetadata::IterableMinimization,
                );
        }
        Ok(())
    }
}
impl<C, E, O, OT, S, I> Restartable<S> for MinimizationStage<C, E, O, OT, S, I> {
    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl::Error> {
        Ok(())
    }
}
