use libafl::{
    corpus::Corpus,
    executors::{Executor, HasObservers},
    feedbacks::{HasObserverHandle, MapIndexesMetadata},
    observers::{MapObserver, ObserversTuple},
    stages::Stage,
    state::{HasCorpus, HasCurrentTestcase, State, UsesState},
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
use autarkie::{MutationType, Node, NodeType, Visitor};

use crate::context::Context;

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
    C: AsRef<O>,
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
    S: State + HasCurrentTestcase + HasCorpus + HasMetadata,
    S::Corpus: Corpus<Input = I>,
    E: Executor<EM, I, S, Z> + HasObservers<Observers = OT>,
    EM: UsesState<State = S>,
    Z: Evaluator<E, EM, I, S>,
    O: MapObserver,
    C: AsRef<O>,
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
        // TODO: check if we need to run this testcase
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
        current.fields(&mut self.visitor.borrow_mut(), 0);
        let mut skip = 0;
        let mut fields = self.visitor.borrow_mut().fields();

        loop {
            let field = fields.pop();
            if field.is_none() {
                break;
            }
            let field = field.unwrap();
            let ((id, node_ty), ty) = field.last().unwrap();
            if let NodeType::Iterable(field_len, inner_ty) = node_ty {
                let path = VecDeque::from_iter(field.iter().map(|(i, ty)| i.0));
                let mut len = *field_len;
                let mut counter = 0;
                loop {
                    if len == 0 || counter >= len {
                        break;
                    }
                    let mut inner = current.clone();
                    inner.__mutate(
                        &mut MutationType::IterablePop(counter),
                        &mut self.visitor.borrow_mut(),
                        path.clone(),
                    );
                    let run = fuzzer.evaluate_input(state, executor, manager, inner.clone())?;
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
                        current = inner;
                        current.fields(&mut self.visitor.borrow_mut(), 0);
                        fields = self.visitor.borrow_mut().fields();
                        len = len.saturating_sub(1);
                    }
                    counter += 1;
                }
            }
        }
        state.current_testcase_mut()?.set_input(current);
        Ok(())
    }

    fn should_restart(&mut self, state: &mut S) -> Result<bool, libafl_bolts::Error> {
        Ok(true)
    }

    fn clear_progress(&mut self, state: &mut S) -> Result<(), libafl_bolts::Error> {
        Ok(())
    }
}

fn contains(a: &Vec<usize>, b: &Vec<usize>) -> bool {
    if b.len() > a.len() {
        return false;
    }
    for (i, item) in a.iter().enumerate() {
        let b_item = b.get(i);
        if let Some(b_item) = b_item {
            if b_item != item {
                return false;
            }
        } else {
            break;
        }
    }
    return true;
}
