#![cfg(feature = "debug")]

use std::sync::{Arc, Mutex};
use tui_dispatch_core::{
    observe::{DispatchObservation, DispatchObserver, DispatchOutcome},
    Action, ReducerResult, Store,
};

#[derive(Clone, Debug)]
struct Add(usize);
impl Action for Add {
    fn name(&self) -> &'static str {
        "Add"
    }
}
// Neither state nor action implements Serialize or DebugState.
struct State(usize);
struct Observer(Arc<Mutex<Vec<(usize, usize, DispatchObservation)>>>);
impl DispatchObserver<State, Add> for Observer {
    fn observe(&mut self, action: &Add, state: &State, observation: DispatchObservation) {
        self.0
            .lock()
            .unwrap()
            .push((action.0, state.0, observation));
    }
}
#[test]
fn core_hook_requires_no_serialization_and_can_be_replaced_or_removed() {
    let first = Arc::new(Mutex::new(Vec::new()));
    let second = Arc::new(Mutex::new(Vec::new()));
    let mut store = Store::new(State(0), |s, a: Add| {
        s.0 += a.0;
        ReducerResult::<()>::changed()
    });
    store.set_observer(Observer(first.clone()));
    store.dispatch(Add(2));
    store.set_observer(Observer(second.clone()));
    store.dispatch(Add(3));
    store.clear_observer();
    store.dispatch(Add(4));
    assert_eq!(
        *first.lock().unwrap(),
        [(
            2,
            2,
            DispatchObservation {
                depth: 0,
                changed: true,
                outcome: DispatchOutcome::Applied
            }
        )]
    );
    assert_eq!(second.lock().unwrap()[0].1, 5);
    assert_eq!(store.state().0, 9);
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    assert_send_sync(&store);
}
