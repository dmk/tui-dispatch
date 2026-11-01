use tui_dispatch_core::{Action, NoEffect, ReducerResult, Store};

#[derive(Debug, PartialEq, Eq)]
enum Effect {
    Child(u8),
    Save,
}

#[test]
fn mapped_effects_keep_change_flags_and_merge_in_order() {
    let child = ReducerResult::effects(vec![1, 2]).mark_changed();
    let result = child
        .map_effect(Effect::Child)
        .merge(ReducerResult::effect(Effect::Save));
    assert!(result.changed);
    assert_eq!(
        result.effects,
        [Effect::Child(1), Effect::Child(2), Effect::Save]
    );
}

#[test]
fn unchanged_identity_and_effect_only_results_do_not_invent_state_changes() {
    let result = ReducerResult::<Effect>::unchanged()
        .merge(ReducerResult::effect(Effect::Save))
        .merge(ReducerResult::unchanged());
    assert!(!result.changed);
    assert_eq!(result.effects, [Effect::Save]);
    let mapped = ReducerResult::<u8>::changed().map_effect(Effect::Child);
    assert!(mapped.changed);
    assert!(mapped.effects.is_empty());
}

#[test]
fn mapping_calls_stateful_mapper_once_per_effect_in_order() {
    // Neither the input nor the output needs Clone, Copy, or Default.
    struct Child(String);
    struct Parent(String, usize);
    let mut seen = Vec::new();
    let result = ReducerResult::effects(vec![Child("a".into()), Child("b".into())]).map_effect(
        |Child(value)| {
            seen.push(value.as_str().to_owned());
            Parent(value, seen.len())
        },
    );
    assert!(!result.changed);
    assert_eq!(seen, ["a", "b"]);
    let values: Vec<_> = result
        .effects
        .into_iter()
        .map(|Parent(s, n)| (s, n))
        .collect();
    assert_eq!(values, [("a".into(), 1), ("b".into(), 2)]);
}

#[test]
fn empty_mapping_preserves_both_change_flags_without_calling_mapper() {
    for changed in [false, true] {
        let mut result = ReducerResult::<u8>::unchanged();
        result.changed = changed;
        let mapped: ReducerResult<String> = result.map_effect(|_| panic!("empty effects"));
        assert_eq!(mapped.changed, changed);
        assert!(mapped.effects.is_empty());
    }
    let result: ReducerResult<Effect> =
        ReducerResult::<NoEffect>::changed().map_effect(|impossible| match impossible {});
    assert!(result.changed);
    assert!(result.effects.is_empty());
}

#[test]
fn merge_ors_all_flag_combinations_independently_of_effects() {
    for left_changed in [false, true] {
        for right_changed in [false, true] {
            for left_effects in [vec![], vec![1, 2]] {
                for right_effects in [vec![], vec![3, 4]] {
                    let expected: Vec<_> =
                        left_effects.iter().chain(&right_effects).copied().collect();
                    let mut left = ReducerResult::effects(left_effects.clone());
                    left.changed = left_changed;
                    let mut right = ReducerResult::effects(right_effects);
                    right.changed = right_changed;
                    let result = left.merge(right);
                    assert_eq!(result.changed, left_changed || right_changed);
                    assert_eq!(result.effects, expected);
                }
            }
        }
    }
}

#[test]
fn merge_is_associative_with_unchanged_identity() {
    let a = ReducerResult::effect(1);
    let b = ReducerResult::changed().with(2);
    let c = ReducerResult::effects(vec![3, 4]);
    assert_eq!(
        a.clone().merge(b.clone()).merge(c.clone()),
        a.clone().merge(b.merge(c))
    );
    assert_eq!(ReducerResult::unchanged().merge(a.clone()), a.clone());
    assert_eq!(a.clone().merge(ReducerResult::unchanged()), a);
}

#[test]
fn merge_reuses_right_vector_for_empty_left_and_spare_left_capacity() {
    let right = ReducerResult::effects(vec![1, 2, 3]);
    let right_ptr = right.effects.as_ptr();
    let result = ReducerResult::changed().merge(right);
    assert_eq!(result.effects.as_ptr(), right_ptr);
    assert!(result.changed);

    let mut effects = Vec::with_capacity(16);
    effects.push(1);
    let left_ptr = effects.as_ptr();
    let result = ReducerResult::effects(effects).merge(ReducerResult::effects(vec![2, 3]));
    assert_eq!(result.effects.as_ptr(), left_ptr);
    assert_eq!(result.effects.capacity(), 16);
    assert_eq!(result.effects, [1, 2, 3]);
}

#[test]
fn composition_moves_large_owned_payloads_without_copying_and_drops_once() {
    use std::{cell::Cell, rc::Rc};
    struct Payload {
        bytes: Vec<u8>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    enum Parent {
        Child(Payload),
    }
    let drops = Rc::new(Cell::new(0));
    let effects: Vec<_> = (0..64)
        .map(|_| Payload {
            bytes: vec![42; 64 * 1024],
            drops: drops.clone(),
        })
        .collect();
    let addresses: Vec<_> = effects.iter().map(|p| p.bytes.as_ptr()).collect();
    let mut tail = effects;
    let head = tail.drain(..32).collect();
    let result = ReducerResult::effects(head)
        .map_effect(Parent::Child)
        .merge(ReducerResult::effects(tail).map_effect(Parent::Child));
    assert!(!result.changed);
    for (Parent::Child(payload), expected) in result.effects.iter().zip(addresses) {
        assert_eq!(payload.bytes.as_ptr(), expected);
        assert_eq!(payload.bytes.len(), 64 * 1024);
    }
    assert_eq!(drops.get(), 0);
    drop(result);
    assert_eq!(drops.get(), 64);
}

#[test]
fn store_dispatch_composes_child_and_parent_effects() {
    #[derive(Clone, Debug)]
    struct Increment;
    impl Action for Increment {
        fn name(&self) -> &'static str {
            "Increment"
        }
    }
    fn child(state: &mut u8) -> ReducerResult<u8> {
        *state += 1;
        ReducerResult::changed().with(*state)
    }
    fn reducer(state: &mut u8, _: Increment) -> ReducerResult<Effect> {
        child(state)
            .map_effect(Effect::Child)
            .merge(ReducerResult::effect(Effect::Save))
    }
    let mut store = Store::new(0, reducer);
    let result = store.dispatch(Increment);
    assert_eq!(*store.state(), 1);
    assert!(result.changed);
    assert_eq!(result.effects, [Effect::Child(1), Effect::Save]);
}
