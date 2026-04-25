---
title: Philosophy
description: Why tui-dispatch exists and how to think about it
---

## Why tui-dispatch exists

Three goals, in priority order:

### 1. Manageable data layer + async

Complex TUI apps have state scattered everywhere - in widgets, in globals, in closures. Bugs hide. Changes cascade unpredictably.

tui-dispatch centralizes state with Redux/Elm patterns: single store, pure reducers, declarative effects. You always know where state lives and how it changes.

### 2. Debuggable by default

Every state change goes through an action. If your state is serializable, you can snapshot it, log the action history, and replay sessions to reproduce bugs.

For best results: keep state serializable (no file handles, no `Rc<RefCell<...>>`), and route side effects through the effect system. The framework provides derive macros and patterns that make this natural.

The debug overlay and replay tools exist because this architecture makes them possible - not the other way around.

### 3. Reduce boilerplate

Derive macros for actions and component IDs. Prebuilt components for common UI patterns. Runtime helpers that wire up the event loop. The goal is less ceremony for common cases.

This is ongoing work. If you find yourself writing the same pattern repeatedly, that's a gap in the framework.

## When not to use tui-dispatch

- Simple single-screen apps with minimal state
- Scripts that just render output and exit
- Apps where state is naturally local to each widget with no sharing
- Teams unfamiliar with Redux/Elm who need to ship quickly

## Architecture: Core + Extensions

tui-dispatch follows a layered architecture. The core is minimal; if you want the bare minimum, use `tui-dispatch-core`.
The `tui-dispatch` crate is a batteries-included facade that re-exports core + debug + macros.

### Layer 0: Core primitives

The bare minimum for state management:

- **Store**: Holds state, dispatches actions
- **Reducer**: Pure function `(state, action) -> changed`
- **Action**: Describes what happened

This is all you need to get started. Everything below is optional.

### Layer 1: Runtime and optional extensions

Plug in what your app needs:

| Extension | Purpose |
|-----------|---------|
| **Runtime** | Event/action/render loop (`DispatchRuntime`, `EffectRuntime`) |
| **Effects** | Declare async work as data (`ReducerResult`, `EffectStore`, `EffectRuntime`) |
| **EventBus** | Event routing (modal → hovered → focused → subscribers → global) |
| **DataResource** | Typed async lifecycle (Empty/Loading/Loaded/Failed) |
| **TaskManager** | Async task lifecycle |
| **Subscriptions** | Stream management |
| **Debug layer** | Frame freeze, state inspection, action log, replay |

Each extension has a consistent pattern: create it and wire it where needed.
State-centric pieces like `DataResource` live in `AppState`, while runtime helpers like `TaskManager` and `Subscriptions`
live in the runtime and are accessed via `EffectContext`.

The runtime composes additively. A base `DispatchRuntime` / `EffectRuntime` can
be extended with `.with_debug(...)` and `.with_event_bus(bus, keybindings)` to
produce a `BusDispatchRuntime` / `BusEffectRuntime` with a fluent `run(...)` /
`run_with_hooks(...)` loop. Apps can still stop at `Layer 0` and write their
own loop when runtime helpers are not wanted.

### Layer 2: Components

The `tui-dispatch-components` crate provides reusable UI building blocks. This layer is still optional.

Think of Layer 2 as three progressively richer tools:

- **View components** via `Component<A>` for minimal props-driven reusable views
- **Interactive widgets** via `InteractiveComponent<A, Ctx>` when local UI state needs routed input and `needs_render`
- **Component host** via `ComponentHost<S, A, Id, Ctx>` when you want long-lived mounted widgets and optional EventBus binding

`ComponentHost` sits at Layer 2 rather than inside the runtime: it wraps the
Layer 1 runtime by consuming the `run_with_hooks(...)` post-render seam to
keep bus areas in sync. That keeps `tui-dispatch-core` component-agnostic.

You can stop at any layer. Many apps never need more than render functions or a few plain view components.
