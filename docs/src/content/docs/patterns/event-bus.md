---
title: Event Bus
description: Route input through EventBus with state-driven focus
---

EventBus is the canonical input router. Instead of writing `map_event`, you register handlers for components and let the bus decide routing order (modal, hovered, focused, subscribers, global).

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

### 4) Use run_with_bus

```rust
runtime
    .run_with_bus(
        terminal,
        &mut bus,
        &keybindings,
        |frame, area, state, render_ctx, event_ctx| {
            event_ctx.set_component_area(AppComponentId::Main, area);
            render_app(frame, area, state, render_ctx);
        },
        |action| matches!(action, Action::Quit),
    )
    .await?;
```

## Routing order

- Modal (if any)
- Hovered (mouse/scroll only)
- Focused
- Subscribers
- Global handlers

Resize and Tick events broadcast to subscribers (Resize/Tick) and global handlers regardless of `consumed`.
