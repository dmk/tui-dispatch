use tui_dispatch_core::prelude::{RefreshableResource, ResourceRequestId};

// Deliberately neither Clone nor Default: lifecycle operations need neither.
#[derive(Debug, PartialEq, Eq)]
enum FetchError {
    Offline,
    Timeout,
}

#[test]
fn refresh_failure_preserves_data_and_typed_error() {
    let mut resource = RefreshableResource::<String, FetchError>::loaded("cached".into());
    let request = resource.begin();
    assert!(resource.is_loading());
    assert_eq!(resource.data().map(String::as_str), Some("cached"));
    assert!(resource.complete(request, Err(FetchError::Offline)));
    assert!(!resource.is_loading());
    assert_eq!(resource.data().map(String::as_str), Some("cached"));
    assert_eq!(resource.error(), Some(&FetchError::Offline));
}

#[test]
fn superseded_and_cancelled_requests_cannot_overwrite_current_data() {
    let mut resource = RefreshableResource::<i32, FetchError>::loaded(1);
    let old = resource.begin();
    let current = resource.begin();
    assert_ne!(old, current);
    assert!(!resource.complete(old, Ok(99)));
    assert!(!resource.complete(old, Err(FetchError::Timeout)));
    assert!(resource.is_loading());
    assert_eq!(resource.data(), Some(&1));
    assert_eq!(resource.error(), None);
    assert!(resource.complete(current, Ok(2)));
    assert_eq!(resource.data(), Some(&2));
    let cancelled = resource.begin();
    assert!(resource.cancel());
    assert!(!resource.cancel());
    assert!(!resource.complete(cancelled, Ok(3)));
    assert_eq!(resource.data(), Some(&2));
    assert!(!resource.is_loading());

    let next = resource.begin();
    assert_ne!(next, cancelled);
    assert!(!resource.complete(cancelled, Err(FetchError::Offline)));
    assert!(resource.is_loading());
    assert!(resource.complete(next, Ok(4)));
    assert_eq!(resource.data(), Some(&4));
}

#[test]
fn initial_failure_retry_and_success_have_independent_data_and_error() {
    let mut resource = RefreshableResource::<i32, FetchError>::default();
    assert_eq!(resource.data(), None);
    assert_eq!(resource.error(), None);
    assert!(!resource.is_loading());
    let request: ResourceRequestId = resource.begin();
    assert!(resource.is_loading());
    assert_eq!(resource.data(), None);
    assert!(resource.complete(request, Err(FetchError::Offline)));
    assert_eq!(resource.data(), None);
    assert_eq!(resource.error(), Some(&FetchError::Offline));
    assert!(!resource.is_loading());
    assert!(!resource.cancel());
    assert_eq!(resource.error(), Some(&FetchError::Offline));

    let retry = resource.begin();
    assert_eq!(resource.error(), None);
    assert!(resource.complete(retry, Ok(42)));
    assert_eq!(resource.data(), Some(&42));
    assert_eq!(resource.error(), None);
    assert!(!resource.is_loading());
}

#[test]
fn duplicate_completions_cannot_change_success_or_failure() {
    let mut resource = RefreshableResource::<i32, FetchError>::empty();
    let success = resource.begin();
    assert!(resource.complete(success, Ok(1)));
    assert!(!resource.complete(success, Ok(2)));
    assert!(!resource.complete(success, Err(FetchError::Timeout)));
    assert_eq!(resource.data(), Some(&1));
    assert_eq!(resource.error(), None);

    let failure = resource.begin();
    assert!(resource.complete(failure, Err(FetchError::Offline)));
    assert!(!resource.complete(failure, Ok(3)));
    assert!(!resource.complete(failure, Err(FetchError::Timeout)));
    assert_eq!(resource.data(), Some(&1));
    assert_eq!(resource.error(), Some(&FetchError::Offline));
}

#[test]
fn cancellation_before_initial_load_keeps_resource_empty() {
    let mut resource = RefreshableResource::<i32, FetchError>::empty();
    let cancelled = resource.begin();
    assert!(resource.cancel());
    assert!(!resource.complete(cancelled, Err(FetchError::Offline)));
    assert_eq!(resource.data(), None);
    assert_eq!(resource.error(), None);
    assert!(!resource.is_loading());
}

#[test]
fn refresh_keeps_large_non_clone_payload_in_place_and_drops_stale_results() {
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
    let drops = Rc::new(Cell::new(0));
    let mut resource = RefreshableResource::<_, FetchError>::loaded(Payload {
        bytes: vec![42; 8 * 1024 * 1024],
        drops: drops.clone(),
    });
    let address = resource.data().unwrap() as *const Payload;
    let allocation = resource.data().unwrap().bytes.as_ptr();
    for _ in 0..100 {
        let stale = resource.begin();
        let current = resource.begin();
        assert!(!resource.complete(
            stale,
            Ok(Payload {
                bytes: vec![0; 1024],
                drops: drops.clone(),
            })
        ));
        assert!(resource.complete(current, Err(FetchError::Offline)));
        assert_eq!(resource.data().unwrap() as *const Payload, address);
        assert_eq!(resource.data().unwrap().bytes.as_ptr(), allocation);
    }
    assert_eq!(drops.get(), 100);
    let current = resource.begin();
    assert!(resource.complete(
        current,
        Ok(Payload {
            bytes: vec![1],
            drops: drops.clone(),
        })
    ));
    assert_eq!(drops.get(), 101);
    drop(resource);
    assert_eq!(drops.get(), 102);
}

#[cfg(feature = "serde")]
#[test]
fn serialization_preserves_pending_ids_and_generation_across_all_states() {
    fn round_trip(resource: &mut RefreshableResource<i32>) {
        let json = serde_json::to_string(resource).unwrap();
        let restored = serde_json::from_str(&json).unwrap();
        assert_eq!(*resource, restored);
        *resource = restored;
    }

    let mut resource = RefreshableResource::empty();
    round_trip(&mut resource);
    let old = resource.begin();
    let current = resource.begin();
    round_trip(&mut resource);
    let id_json = serde_json::to_string(&current).unwrap();
    let current = serde_json::from_str(&id_json).unwrap();
    assert!(!resource.complete(old, Ok(0)));
    assert!(resource.complete(current, Ok(1)));
    round_trip(&mut resource);
    let failed = resource.begin();
    assert!(resource.complete(failed, Err("offline".into())));
    round_trip(&mut resource);
    let cancelled = resource.begin();
    resource.cancel();
    round_trip(&mut resource);
    let next = resource.begin();
    assert_ne!(next, cancelled);
    assert!(!resource.complete(cancelled, Ok(2)));
    assert!(resource.complete(next, Ok(3)));
}

#[cfg(feature = "json-schema")]
#[test]
fn resources_and_request_ids_can_be_used_in_state_and_action_schemas() {
    let resource = schemars::schema_for!(RefreshableResource<String, String>);
    let request = schemars::schema_for!(ResourceRequestId);
    assert!(resource.as_object().unwrap().contains_key("properties"));
    assert_eq!(request.as_object().unwrap().get("type").unwrap(), "integer");
}
