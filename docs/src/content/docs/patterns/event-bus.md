---
title: Event Bus
description: Route input through EventBus with state-driven focus
---

For apps with multiple focusable components, EventBus routes input based on focus state. You register handlers for components and the bus decides routing order (modal → hovered → focused → subscribers → global).

For simple single-component apps, you can handle events directly in your main loop without EventBus—see the [minimal example](/tui-dispatch/getting-started/quick-start/#minimal-example-counter).

## Quick Start: SimpleEventBus

For apps that don't need context-specific keybindings, use `SimpleEventBus` with `DefaultBindingContext`:

```rust
use tui_dispatch::{SimpleEventBus, DefaultBindingContext, EventRoutingState, Keybindings};

#[derive(tui_dispatch::ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum AppComponentId {
    Main,
}

impl EventRoutingState<AppComponentId, DefaultBindingContext> for AppState {
    fn focused(&self) -> Option<AppComponentId> { Some(AppComponentId::Main) }
    fn modal(&self) -> Option<AppComponentId> { None }
    fn binding_context(&self, _id: AppComponentId) -> DefaultBindingContext { DefaultBindingContext }
    fn default_context(&self) -> DefaultBindingContext { DefaultBindingContext }
}

let mut bus: SimpleEventBus<AppState, Action, AppComponentId> = SimpleEventBus::new();
let keybindings: Keybindings<DefaultBindingContext> = Keybindings::new();
```

This reduces boilerplate by ~10 lines compared to defining a custom context enum.

## Full API: Custom Binding Contexts

For apps with multiple modes (e.g., normal/search/modal), define custom contexts:

### 1) Define routing types

```rust
#[derive(tui_dispatch::ComponentId, Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum AppComponentId {
    Main,
    Search,
}

#[derive(tui_dispatch::BindingContext, Clone, Copy, PartialEq, Eq, Hash)]
enum AppContext {
    Main,
    Search,
}
```

### 2) Implement EventRoutingState on your app state

```rust
impl EventRoutingState<AppComponentId, AppContext> for AppState {
    fn focused(&self) -> Option<AppComponentId> {
        if self.search_open { Some(AppComponentId::Search) } else { Some(AppComponentId::Main) }
    }

    fn modal(&self) -> Option<AppComponentId> {
        if self.search_open { Some(AppComponentId::Search) } else { None }
    }

    fn binding_context(&self, id: AppComponentId) -> AppContext {
        match id {
            AppComponentId::Main => AppContext::Main,
            AppComponentId::Search => AppContext::Search,
        }
    }

    fn default_context(&self) -> AppContext {
        AppContext::Main
    }
}
```

### 3) Register handlers

```rust
let mut bus: EventBus<AppState, Action, AppComponentId, AppContext> = EventBus::new();
let keybindings: Keybindings<AppContext> = Keybindings::new();

bus.register(AppComponentId::Main, |event, state| {
    let actions = handle_main_event(&event.kind, state);
    if actions.is_empty() {
        HandlerResponse::ignored()
    } else {
        HandlerResponse::actions(actions)
    }
});

bus.register_global(|event, _state| match event.kind {
    EventKind::Resize(_, height) => {
        HandlerResponse::action(Action::UiTerminalResize(height)).with_render()
    }
    _ => HandlerResponse::ignored(),
});
```

## HandlerResponse

`HandlerResponse` controls propagation:

```rust
HandlerResponse::ignored()              // No action, event continues routing
HandlerResponse::action(action)         // One action, event consumed
HandlerResponse::actions(vec)           // Multiple actions, event consumed
HandlerResponse::actions_passthrough(v) // Multiple actions, event continues routing
    .with_render()                      // Force render even if state unchanged
    .with_consumed(true)                // Explicitly set consumed flag
```

### 4) Compose the runtime with `with_event_bus`

Attach the bus and keybindings to your runtime, then `run(...)` it:

```rust
let mut runtime = Runtime::new(AppState::default(), reducer)
    .with_event_bus(bus, keybindings);

runtime
    .run(
        terminal,
        |frame, area, state, render_ctx, event_ctx| {
            event_ctx.set_component_area(AppComponentId::Main, area);
            render_app(frame, area, state, render_ctx);
        },
        |action| matches!(action, Action::Quit),
    )
    .await?;
```

`with_event_bus(...)` keeps the value as a `Runtime` with bus routing enabled.
It exposes `bus()`, `bus_mut()`, `keybindings()`,
`enqueue(...)`, and `action_tx()` for pre-`run` setup or outside-the-loop
wiring.

#### `run_with_hooks` for post-render wiring

When a wrapper needs to touch the bus after each frame, use
`run_with_hooks(...)` instead:

```rust
runtime
    .run_with_hooks(
        terminal,
        render,
        is_quit,
        |bus, state| host.sync_areas(bus),
    )
    .await?;
```

This is the stable Layer 2 integration seam.

For `ComponentHost`, prefer the components-layer runtime adapter:

```rust
use tui_dispatch_components::RuntimeHostExt;

let mut runtime = Runtime::new(state, reducer)
    .with_event_bus(bus, keybindings)
    .with_component_host(host.clone());

runtime
    .run(terminal, render, is_quit)
    .await?;
```

The hosted runtime consumes the same hook internally without any changes in
core.

## Configuring Global Keys

By default, Esc, Ctrl+C, and Ctrl+Q are treated as "global" events — they bypass modal blocking and reach global subscribers. This can be surprising for apps that use Esc to close modals.

Use `GlobalKeyPolicy` to customize this behavior:

```rust
use tui_dispatch::{EventBus, GlobalKeyPolicy};

// Default: Esc, Ctrl+C, Ctrl+Q are global
let bus = EventBus::new();

// Remove Esc from global keys (keep Ctrl+C, Ctrl+Q)
let bus = EventBus::new()
    .with_global_key_policy(GlobalKeyPolicy::without_esc());

// Only Ctrl+C is global
let bus = EventBus::new()
    .with_global_key_policy(GlobalKeyPolicy::keys(vec![
        (KeyCode::Char('c'), KeyModifiers::CONTROL),
    ]));

// No key events are global (only Resize remains global)
let bus = EventBus::new()
    .with_global_key_policy(GlobalKeyPolicy::none());

// Custom predicate
let bus = EventBus::new()
    .with_global_key_policy(GlobalKeyPolicy::custom(|event| {
        matches!(event, EventKind::Key(k) if k.modifiers.contains(KeyModifiers::CONTROL))
    }));
```

Resize events are always global regardless of the policy.

## Routing order

- Modal (if any)
- Hovered (mouse/scroll only)
- Focused
- Subscribers
- Global subscribers (global events only)
- Global handlers (always last)

Modal blocks non-broadcast events from reaching hovered/focused/subscribers.
Global subscribers only receive **global** events.
Global handlers run last and still run for global events even if a modal is active (they are skipped for non-global events when modal blocks).
Resize and Tick events broadcast to subscribers (Resize/Tick) and global handlers regardless of `consumed`.
