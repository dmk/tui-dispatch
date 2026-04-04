---
title: Interactive Widgets
description: Use `InteractiveComponent` for routed input and local widget state
---

`InteractiveComponent<A, Ctx>` is the richer widget contract used by stateful reusable widgets like `TextInput`, `SelectList`, `ScrollView`, and `TreeView`.

Use it when plain `Component<A>` stops being expressive enough.

## What It Adds

Compared to `Component<A>`, interactive widgets get:

- `ComponentInput` instead of raw `EventKind`
- `HandlerResponse<A>` instead of just actions
- `needs_render` for local-state-only updates
- a natural integration point for `ComponentHost`

## The Trait

```rust
pub trait InteractiveComponent<A, Ctx = DefaultBindingContext> {
    type Props<'a> where Self: 'a;

    fn update(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: Self::Props<'_>,
    ) -> HandlerResponse<A> {
        HandlerResponse::ignored()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);
}
```

## Routed Input

`ComponentInput` keeps semantic commands and raw input in the same model:

```rust
pub enum ComponentInput<'a, Ctx> {
    Command { name: &'a str, ctx: Ctx },
    Key(KeyEvent),
    Mouse(MouseEvent),
    Scroll { .. },
    Resize(u16, u16),
    Tick,
}
```

Routing rule:

- if keybindings resolve a command, the widget receives `Command`
- otherwise it receives the raw `Key`

That lets a list react to `next` or `select` while a text input still sees literal characters.

## Why `HandlerResponse` Matters

Interactive widgets can say more than “here are some actions”.

They can also say:

- whether they consumed the event
- whether the UI should re-render even if app state did not change

That matters for local widget mechanics like:

- moving a text cursor
- updating a scroll offset
- expanding a tree row

## Direct Usage Without a Host

You do not need `ComponentHost` to use an interactive widget.

```rust
use tui_dispatch_components::{
    ComponentInput, InteractiveComponent, TextInput, TextInputProps, TextInputRenderProps,
    TextInputStyle,
};

#[derive(Clone, Debug)]
enum Action {
    QueryChanged(String),
    Submit,
}

let mut input = TextInput::new();

let response = <TextInput as InteractiveComponent<Action>>::update(
    &mut input,
    ComponentInput::Key(key_event),
    TextInputProps {
        value: &state.query,
        placeholder: "Type to search",
        is_focused: true,
        style: TextInputStyle::default(),
        on_change: Action::QueryChanged,
        on_submit: |_| Action::Submit,
        on_cursor_move: None,
    },
);

for action in response.actions {
    store.dispatch(action);
}

if response.needs_render {
    // schedule or perform a render
}

input.render_widget(
    frame,
    area,
    TextInputRenderProps {
        value: &state.query,
        placeholder: "Type to search",
        is_focused: true,
        style: TextInputStyle::default(),
    },
);
```

This pattern already solves the biggest limitation of plain `Component<A>`: local widget state can request a render without inventing fake app actions.

## Built-In Interactive Widgets

These widgets implement `InteractiveComponent<A, Ctx>` today:

- `TextInput`
- `SelectList`
- `ScrollView`
- `TreeView`

They also expose render-only helpers like `render_widget(...)` when you want to separate input handling from rendering.

## Composing Interactive Widgets

Interactive widgets are especially useful as building blocks for your own reusable widgets.

Typical pattern:

1. your widget owns one or more child widgets
2. `update(...)` delegates input to those children
3. your widget translates child-local actions into your app actions or local state changes
4. `render(...)` draws the composed result

That is the pattern used by the `log-viewer` example's filter panes and detail viewer.

## State Ownership

Interactive widgets still follow the same boundary:

Keep in app state:

- query text
- selected ids
- modes and filters
- loaded data

Keep local to the widget:

- cursor position
- viewport height
- scroll offset
- temporary expansion bookkeeping

Interactive widgets make the local side ergonomic, but they do not replace your store.

## When to Use This Layer

Use `InteractiveComponent` when:

- widgets react to semantic commands and raw keys
- local UI mechanics need `needs_render`
- you are composing pre-built widgets into a larger reusable widget

Stay with `Component<A>` when:

- raw `EventKind` is enough
- the surrounding event wiring is small
- local-state-only renders are not a concern

Move up to [Component Host](/tui-dispatch/components/host/) when you want mounted instances and one-time props wiring.
