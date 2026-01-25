---
title: Building Custom Components
description: How to build your own components using the Component trait
---

This guide covers how to build your own components using the `Component<A>` trait from tui-dispatch-core. If you're familiar with [React's props pattern](https://react.dev/learn/passing-props-to-a-component) or [Elm's view functions](https://guide.elm-lang.org/architecture/buttons), the concepts will feel natural.

## The Component Trait

```rust
pub trait Component<A> {
    /// Data required to render the component (read-only)
    type Props<'a>;

    /// Handle an event and return actions to dispatch
    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        None::<A>  // Default: render-only component
    }

    /// Render the component to the frame
    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);
}
```

## Design Principles

Components follow these rules:

1. **Props contain ALL data** - Everything the component needs to render comes through props
2. **Focus via props** - Components receive `EventKind`, not the full `Event` with focus context
3. **Actions, not mutations** - `handle_event` returns actions; it never mutates external state
4. **Internal state is OK** - Scroll position, cursor position, animation state can live in `&mut self`

## Basic Example: Counter

A minimal component that handles keypresses and renders a count:

```rust
use crossterm::event::KeyCode;
use ratatui::{Frame, layout::Rect, widgets::Paragraph};
use tui_dispatch::{Component, EventKind};

// The component struct - can hold internal UI state
struct Counter;

// Props define what data the component needs
struct CounterProps {
    count: i32,
    is_focused: bool,
}

// Your app's action type
#[derive(Clone, Debug)]
enum Action {
    Increment,
    Decrement,
}

impl Component<Action> for Counter {
    type Props<'a> = CounterProps;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = Action> {
        // Ignore events when not focused
        if !props.is_focused {
            return None;
        }

        if let EventKind::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => return Some(Action::Increment),
                KeyCode::Down | KeyCode::Char('j') => return Some(Action::Decrement),
                _ => {}
            }
        }
        None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let text = format!("Count: {}", props.count);
        frame.render_widget(Paragraph::new(text), area);
    }
}
```

## Checklist: Your First Component

Quick reference for building a new component:

1. **Define Props** - What read-only data does the component need?
   ```rust
   type Props<'a> = &'a MyData;
   // Or with multiple fields:
   type Props<'a> = MyProps<'a>;
   ```

2. **Implement `handle_event`** - Return actions, never mutate app state
   ```rust
   fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>)
       -> impl IntoIterator<Item = A>
   ```

3. **Implement `render`** - Pure rendering from props
   ```rust
   fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>)
   ```

4. **Handle focus** - Check `props.is_focused` before processing keys

5. **Internal state** - Cursor position, scroll offset go in `&mut self`

6. **Test with `key()` helper**:
   ```rust
   use tui_dispatch::testing::key;
   let event = key("k").into();
   let actions: Vec<_> = component.handle_event(&event, props).into_iter().collect();
   ```

## Focus Handling Pattern

Components don't know about focus management - they just check `is_focused` in props:

```rust
struct MyProps<'a> {
    data: &'a str,
    is_focused: bool,  // Caller determines this
}

fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> impl IntoIterator<Item = A> {
    // Early return if not focused
    if !props.is_focused {
        return None;
    }
    // ... handle event
}
```

The parent (or runtime) determines which component is focused:

```rust
// In your EventBus handler
bus.register(ComponentId::MyComponent, |event, state| {
    let props = MyProps {
        data: &state.data,
        is_focused: state.focus == Focus::MyComponent,
    };
    let actions: Vec<_> = component.handle_event(&event.kind, props).into_iter().collect();
    if actions.is_empty() {
        HandlerResponse::ignored()
    } else {
        HandlerResponse {
            actions,
            consumed: true,
            needs_render: false,
        }
    }
});
```

## Props with Action Constructors

For reusable components, pass action constructors through props:

```rust
struct SearchBoxProps<'a> {
    value: &'a str,
    is_focused: bool,
    // Action constructors - caller defines what actions to emit
    on_change: fn(String) -> Action,
    on_submit: fn(String) -> Action,
}

impl Component<Action> for SearchBox {
    type Props<'a> = SearchBoxProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> impl IntoIterator<Item = Action> {
        if let EventKind::Key(key) = event {
            match key.code {
                KeyCode::Enter => {
                    return Some((props.on_submit)(props.value.to_string()));
                }
                KeyCode::Char(c) => {
                    let new_value = format!("{}{}", props.value, c);
                    return Some((props.on_change)(new_value));
                }
                _ => {}
            }
        }
        None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        // ...
    }
}
```

Usage:

```rust
let props = SearchBoxProps {
    value: &state.query,
    is_focused: true,
    on_change: Action::SearchQueryChange,
    on_submit: Action::SearchSubmit,
};
```

## Internal State

Components can hold internal UI state that doesn't affect app logic:

```rust
struct ScrollableList {
    scroll_offset: usize,  // Internal - doesn't need to go through actions
}

impl Component<Action> for ScrollableList {
    type Props<'a> = ListProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> impl IntoIterator<Item = Action> {
        if let EventKind::Key(key) = event {
            match key.code {
                // Internal scroll - no action needed
                KeyCode::PageDown => {
                    self.scroll_offset = (self.scroll_offset + 10).min(props.items.len());
                    return None;  // Still need render, but no action
                }
                // Selection change - needs action
                KeyCode::Enter => {
                    return Some((props.on_select)(self.get_selected_index()));
                }
                _ => {}
            }
        }
        None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        // Use self.scroll_offset to determine what to render
    }
}
```

**When to use internal state:**
- Scroll position, cursor position
- Animation frame counters
- Hover state, temporary highlights

**When to use actions:**
- Selection changes
- Value changes (text input)
- Navigation to other views

## Composing Components

Components can contain other components:

```rust
struct SearchDialog {
    input: TextInput,
    list: SelectList,
}

impl Component<Action> for SearchDialog {
    type Props<'a> = SearchDialogProps<'a>;

    fn handle_event(&mut self, event: &EventKind, props: Self::Props<'_>) -> impl IntoIterator<Item = Action> {
        // Delegate to child based on state
        if props.results.is_empty() {
            // No results yet - input handles all events
            let input_props = TextInputProps { /* ... */ };
            self.input.handle_event(event, input_props).into_iter().collect()
        } else {
            // Have results - check if list wants the event
            match event {
                EventKind::Key(key) if matches!(key.code, KeyCode::Up | KeyCode::Down) => {
                    let list_props = SelectListProps { /* ... */ };
                    self.list.handle_event(event, list_props).into_iter().collect()
                }
                _ => {
                    let input_props = TextInputProps { /* ... */ };
                    self.input.handle_event(event, input_props).into_iter().collect()
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let chunks = Layout::vertical([
            Constraint::Length(3),  // Input
            Constraint::Min(1),     // List
        ]).split(area);

        let input_props = TextInputProps { /* ... */ };
        self.input.render(frame, chunks[0], input_props);

        let list_props = SelectListProps { /* ... */ };
        self.list.render(frame, chunks[1], list_props);
    }
}
```

## Return Types from handle_event

The `handle_event` method returns `impl IntoIterator<Item = A>`, which accepts:

```rust
// No actions
None

// Single action
Some(Action::DoSomething)

// Multiple actions (array)
[Action::First, Action::Second]

// Multiple actions (vec)
vec![Action::First, Action::Second]

// Conditional
if condition { Some(action) } else { None }
```

## Render-Only Components

Components that don't handle events can skip `handle_event` (uses default):

```rust
struct StatusDisplay;

impl Component<Action> for StatusDisplay {
    type Props<'a> = &'a AppState;

    // handle_event uses default (returns None)

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let status = format!("Items: {} | Mode: {:?}", props.items.len(), props.mode);
        frame.render_widget(Paragraph::new(status), area);
    }
}
```

## Integration with Runtime

Components work with `EffectRuntime` or `DispatchRuntime`:

```rust
let mut ui = MyComponent::new();

EffectRuntime::new(state, reducer)
    .run(
        terminal,
        // Render
        |frame, area, state, _ctx| {
            let props = MyProps { data: state, is_focused: true };
            ui.render(frame, area, props);
        },
        // Map events
        |event, state| {
            let props = MyProps { data: state, is_focused: true };
            EventOutcome::from_actions(ui.handle_event(event, props))
        },
        |action| matches!(action, Action::Quit),
        handle_effect,
    )
    .await?;
```

## Testing Components

Use the testing utilities to verify component behavior:

```rust
use tui_dispatch::testing::{key, keys};

#[test]
fn test_counter_increment() {
    let mut component = Counter;
    let props = CounterProps { count: 0, is_focused: true };

    // Create a key event and collect actions
    let event = key("k").into();
    let actions: Vec<_> = component.handle_event(&event, props).into_iter().collect();

    assert_eq!(actions, vec![Action::Increment]);
}

#[test]
fn test_ignores_when_unfocused() {
    let mut component = Counter;
    let props = CounterProps { count: 0, is_focused: false };

    let event = key("k").into();
    let actions: Vec<_> = component.handle_event(&event, props).into_iter().collect();

    assert!(actions.is_empty());
}
```

For testing reducers and full dispatch flows, use `StoreTestHarness`:

```rust
use tui_dispatch::testing::StoreTestHarness;

#[test]
fn test_counter_state() {
    let mut harness = StoreTestHarness::new(AppState::default(), reducer);

    harness.dispatch(Action::Increment);
    assert_eq!(harness.state().count, 1);

    harness.dispatch(Action::Decrement);
    assert_eq!(harness.state().count, 0);
}
```

### Testing Async Effects

For components that trigger effects, use `EffectStoreTestHarness`:

```rust
use tui_dispatch::testing::{EffectAssertions, EffectStoreTestHarness};

#[test]
fn test_fetch_triggers_effect() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Dispatch intent action
    harness.dispatch_collect(Action::Fetch);

    // Verify loading state
    harness.assert_state(|s| s.is_loading);

    // Verify effect was emitted
    let effects = harness.drain_effects();
    effects.effects_count(1);
    effects.effects_first_matches(|e| matches!(e, Effect::FetchData));
}

#[test]
fn test_did_load_updates_state() {
    let mut harness = EffectStoreTestHarness::new(AppState::default(), reducer);

    // Simulate async completion
    harness.dispatch_collect(Action::DidLoad("data".into()));

    harness.assert_state(|s| !s.is_loading);
    harness.assert_state(|s| s.data.is_some());
}
```

## Summary

| Concept | Where it lives |
|---------|----------------|
| App data | Props (read-only) |
| Focus state | Props (`is_focused: bool`) |
| Action constructors | Props (`on_change: fn(T) -> A`) |
| Scroll/cursor position | Component struct (`&mut self`) |
| State mutations | Actions returned from `handle_event` |

## See Also

- [Pre-built Components](/tui-dispatch/components/prebuilt/) - Ready-to-use components from `tui-dispatch-components`
- [Core Concepts: Component](/tui-dispatch/getting-started/core-concepts/#component) - Definition and quick example
