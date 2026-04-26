---
title: Component Host
description: Mount long-lived widgets and optionally bind them into EventBus
---

`ComponentHost<S, A, Id, Ctx>` is an optional Layer 2 helper for long-lived interactive widgets. It wraps the Layer 1 runtime additively through `RuntimeHostExt::with_component_host(...)` — `tui-dispatch-core` stays component-agnostic.

It solves two related problems:

- where reusable widget instances live
- how to avoid rebuilding the same props in both event wiring and render wiring

The host owns widget instances and their props factories. Your store still owns app state.

## When to Use It

Reach for `ComponentHost` when:

- you have multiple focusable widgets
- those widgets keep local UI state across frames
- the same props logic is repeated in render and event handlers
- you want optional EventBus integration without storing widgets ad hoc in UI state

If you do not have that pressure yet, stay with direct `InteractiveComponent::update(...)` calls.

## The Core Idea

Mount a widget once:

```rust
use std::rc::Rc;
use tui_dispatch_components::{PropsFactory, TextInputCallback};

struct SearchPropsFactory {
    on_change: TextInputCallback<Action>,
    on_submit: TextInputCallback<Action>,
}

impl PropsFactory<AppState, TextInput, Action, AppBindingContext> for SearchPropsFactory {
    fn props<'a>(&self, state: &'a AppState) -> TextInputProps<'a, Action> {
        TextInputProps {
            value: &state.query,
            placeholder: "Search",
            is_focused: state.focus == Focus::Search,
            style: TextInputStyle::default(),
            on_change: self.on_change.clone(),
            on_submit: self.on_submit.clone(),
            on_cursor_move: None,
            on_cancel: None,
        }
    }
}

let host = ComponentHost::<AppState, Action, RouteId, AppBindingContext>::new();
let search = host.mount(
    TextInput::new,
    SearchPropsFactory {
        on_change: Rc::new(Action::QueryChanged),
        on_submit: Rc::new(|_| Action::SubmitSearch),
    },
);
```

After that, the same stored props factory is reused for:

- `host.update(...)`
- `host.render(...)`

That is the main ergonomics win.

## EventBus Integration

The host can optionally bind mounted widgets to EventBus routes:

```rust
let host = ComponentHost::<AppState, Action, RouteId, AppBindingContext>::new();
let mounted = MountedViews {
    levels: host.mount(FilterPane::new, level_filter_props),
    tags: host.mount(FilterPane::new, tag_filter_props),
    logs: host.mount(SelectList::new, log_list_props),
    details: host.mount(LogDetails::new, details_props),
};

let mut bus = EventBus::<AppState, Action, RouteId, AppBindingContext>::new();
host.bind(&mut bus, RouteId::Levels, mounted.levels);
host.bind(&mut bus, RouteId::Tags, mounted.tags);
host.bind(&mut bus, RouteId::Logs, mounted.logs);
host.bind(&mut bus, RouteId::Details, mounted.details);
```

`bind(...)` registers the widget handler and subscribes it to the event types
declared by `InteractiveComponent::subscriptions()`. Widgets default to key
events; override the associated function when a component needs broadcast-style
delivery such as ticks or resizes.

Render stays explicit:

```rust
terminal.draw(|frame| {
    host.render(mounted.levels, frame, levels_area, store.state());
    host.render(mounted.tags, frame, tags_area, store.state());
    host.render(mounted.logs, frame, logs_area, store.state());
    host.render(mounted.details, frame, details_area, store.state());
})?;

host.sync_areas(&mut bus);
```

`sync_areas(...)` keeps EventBus hit-testing and routing context aligned with the widgets rendered this frame.

### Wiring into the runtime

When the runtime owns the main loop, attach the host after adding the event bus:

```rust
use tui_dispatch_components::RuntimeHostExt;

let mut runtime = Runtime::new(state, reducer)
    .with_event_bus(bus, keybindings)
    .with_component_host(host.clone());

runtime
    .run(
        terminal,
        render,
        is_quit,
    )
    .await?;
```

This is the recommended Layer 2 integration path: `ComponentHost` lives in
`tui-dispatch-components` and uses the Layer 1 post-render hook internally, so
`tui-dispatch-core` stays component-agnostic. If you still need your own
post-render hook, call `run_with_hooks(...)` on the hosted runtime; it runs
`host.sync_areas(...)` first, then your hook.

## Without EventBus

`ComponentHost` is still useful if you want mounted widgets but do not want bus routing:

```rust
let response = host.update(search, ComponentInput::Key(key_event), store.state());

for action in response.actions {
    store.dispatch(action);
}
```

That keeps the host aligned with the layered architecture instead of making EventBus mandatory.

## Ownership Rules

The architecture is:

- store owns app state
- host owns mounted widget instances and widget-local runtime state
- EventBus optionally routes into the host

For ids managed through `host.bind(...)`, treat the host as the authoritative owner of that binding.

In other words: do not manually re-register the same route on the bus behind the host's back.

## Lifecycle

Important host operations:

- `mount(...)`
- `bind(...)`
- `unbind(...)`
- `unmount(...)`
- `reset_local_state(...)`
- `mounted_components(...)`

`unmount(...)` intentionally refuses to remove a still-bound widget. Unbind it first.

That keeps cleanup explicit instead of leaving stale routing behind.

## Debug and Replay Semantics

Widget-local state inside the host is runtime state.

That means:

- it can be inspected through `mounted_components()`
- it can be reset through `reset_local_state()`
- it is not reconstructed automatically by action-only replay

The canonical replay/debug guarantee remains app state plus action log.

## When This Layer Is Worth It

`ComponentHost` is worth the complexity when it deletes real boilerplate from a larger app.

Typical signs:

- widgets were being stored ad hoc in UI state
- props were built once for input and again for render
- EventBus registration code started to dominate `main.rs`

If none of that is happening yet, stay on the simpler layers.
