---
title: View Components
description: Build minimal props-driven UI with `Component<A>`
---

`Component<A>` is the smallest reusable UI abstraction in `tui-dispatch`.

Use it when you want to package rendering plus simple raw-event handling into a reusable type, but you do not need routed commands, `HandlerResponse`, or host-managed lifecycle.

If you have not read it yet, start with [Components Overview](/tui-dispatch/components/).

## What `Component<A>` Means

`Component<A>` is a props-driven view object:

- props contain all read-only data needed to render
- `handle_event` receives raw `EventKind`
- focus and routing context stay outside the component
- the component can keep local UI mechanics in `&mut self`

That makes it a good fit for small reusable views and for apps where the parent or EventBus handler still owns the surrounding wiring.

## The Trait

```rust
pub trait Component<A> {
    type Props<'a> where Self: 'a;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        None::<A>
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);
}
```

Two constraints matter:

- input is raw `EventKind`, not routed `ComponentInput`
- output is actions only, not `HandlerResponse`

If you need routed commands like `next` / `select` or explicit `needs_render`, use [Interactive Widgets](/tui-dispatch/components/interactive/).

## Example

```rust
use crossterm::event::KeyCode;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use tui_dispatch::{Component, EventKind};

#[derive(Clone, Debug)]
enum Action {
    Increment,
    Decrement,
}

struct Counter;

struct CounterProps {
    count: i32,
    is_focused: bool,
}

impl Component<Action> for Counter {
    type Props<'a> = CounterProps;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = Action> {
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
        frame.render_widget(Paragraph::new(format!("count = {}", props.count)), area);
    }
}
```

## Focus and Routing Stay Outside

`Component<A>` does not know what “focused” means for your app. Pass that through props:

```rust
struct CounterProps {
    count: i32,
    is_focused: bool,
}
```

That keeps the component decoupled from your `ComponentId` enum and your focus policy.

## Manual EventBus Wiring

With plain components, the EventBus handler is still responsible for building props and translating actions into a `HandlerResponse`:

```rust
use tui_dispatch::{EventBus, HandlerResponse};

let mut counter = Counter;

bus.register(AppComponentId::Counter, move |event, state| {
    let props = CounterProps {
        count: state.count,
        is_focused: state.focus == Focus::Counter,
    };

    let actions: Vec<_> = counter.handle_event(&event.kind, props).into_iter().collect();

    if actions.is_empty() {
        HandlerResponse::ignored()
    } else {
        HandlerResponse::actions(actions)
    }
});
```

That pattern is still fine when the wiring is small.

If you find yourself rebuilding the same props in both event handlers and render paths, or you want a mounted long-lived widget instance, move up to [Component Host](/tui-dispatch/components/host/).

## Local State Is Allowed, But Keep the Boundary Clear

Good local state for `Component<A>`:

- scroll offset
- cursor position
- hover or temporary highlight state

Keep semantically meaningful state in the store:

- selected item
- query text
- modes and filters
- loaded data

## Important Limitation: No Built-In Render Hint

`Component<A>` only emits actions. It does not have a standard way to say “I changed local state, please re-render”.

That means:

- if local state changes but you do not emit an action, your surrounding code must decide how to trigger a render
- if this becomes a recurring problem, you are probably in [Interactive Widgets](/tui-dispatch/components/interactive/) territory

## When to Use `Component<A>`

Use it when:

- you want a small reusable view type
- raw `EventKind` is enough
- your app already has clear surrounding routing code
- local-state-only re-rendering is rare or controlled elsewhere

Skip it when:

- you need command-aware input like `next`, `prev`, `select`
- you want `HandlerResponse` and `needs_render`
- you want mounted widget instances with a single stored props factory

## See Also

- [Components Overview](/tui-dispatch/components/)
- [Interactive Widgets](/tui-dispatch/components/interactive/)
- [Component Host](/tui-dispatch/components/host/)
- [Pre-built Widgets](/tui-dispatch/components/prebuilt/)
