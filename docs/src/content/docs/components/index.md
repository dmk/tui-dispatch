---
title: Components Overview
description: Choose the right Layer 2 UI abstraction
---

Layer 2 is optional.

Most apps should start with plain render functions. Add reusable UI abstractions only when repeated wiring or repeated interaction patterns show up.

## Decision Guide

| If you need... | Use... |
| --- | --- |
| App-specific drawing directly from state | Plain render functions |
| A reusable props-driven view that handles raw `EventKind` | `Component<A>` |
| A reusable widget with local UI state, routed commands, or `needs_render` | `InteractiveComponent<A, Ctx>` |
| Long-lived mounted widget instances and one-time props wiring | `ComponentHost<S, A, Id, Ctx>` |

## The Three Layer 2 Concepts

### 1. View Components

`Component<A>` is the minimal reusable UI contract.

It is a good fit when:

- props fully describe rendering
- raw `EventKind` is enough
- the surrounding app still owns routing, focus, and lifecycle

Read: [View Components](/tui-dispatch/components/custom/)

### 2. Interactive Widgets

`InteractiveComponent<A, Ctx>` is the richer widget contract used when a widget needs:

- routed `ComponentInput`
- semantic commands like `next`, `prev`, `select`
- `HandlerResponse`
- `needs_render` for local-state-only updates

Read: [Interactive Widgets](/tui-dispatch/components/interactive/)

### 3. Mounted Widgets

`ComponentHost<S, A, Id, Ctx>` owns long-lived widget instances and their props factories.

Use it when you want:

- widget instances mounted once and reused across frames
- props built once per widget definition instead of duplicated in render and event wiring
- optional EventBus binding through `host.bind(...)`

Read: [Component Host](/tui-dispatch/components/host/)

## Pre-built Widgets

The `tui-dispatch-components` crate ships ready-made widgets like:

- `SelectList`
- `ScrollView`
- `TextInput`
- `TreeView`
- `StatusBar`
- `Modal`

These widgets fit into the model above:

- some can be used render-only
- some implement plain `Component<A>`
- some also implement `InteractiveComponent<A, Ctx>`

Read: [Pre-built Widgets](/tui-dispatch/components/prebuilt/)

## State Ownership

This boundary matters more than any trait choice.

Keep this in the store:

- selected item or selected node
- input value or query text
- active filters and modes
- loaded data and async lifecycle
- anything reducers or effects reason about

Keep this local to the widget:

- cursor position
- scroll offset
- hover state
- cached layout measurements
- temporary highlight or animation frame

If other parts of the app care about it semantically, it probably belongs in the store.

## Debug and Replay Semantics

The canonical source of truth is still:

- action log
- store state

Widget-local state is runtime state:

- it can be inspectable
- it can be resettable
- it is not automatically reconstructed by action-only replay

That tradeoff is intentional.

## Recommended Progression

In practice:

1. Start with a render function.
2. Extract a `Component<A>` if a view becomes reusable.
3. Move to `InteractiveComponent` if local widget mechanics need routed input or render hints.
4. Add `ComponentHost` only when repeated mounting and props wiring become real boilerplate.

This keeps the framework layered instead of making the heaviest path feel mandatory.
