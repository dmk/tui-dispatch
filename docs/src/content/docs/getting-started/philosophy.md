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

tui-dispatch follows a layered architecture. The core is minimal; everything else is opt-in.

### Layer 0: Core primitives

The bare minimum for state management:

- **Store**: Holds state, dispatches actions
- **Reducer**: Pure function `(state, action) -> changed`
- **Action**: Describes what happened

This is all you need to get started. Everything below is optional.

### Layer 1: Extensions

Plug in what your app needs:

| Extension | Purpose |
|-----------|---------|
| **EventBus** | Event routing (modal → focused → global) |
| **DataResource** | Typed async lifecycle (Empty/Loading/Loaded/Failed) |
| **TaskManager** | Async task lifecycle |
| **Subscriptions** | Stream management |

Each extension has a consistent pattern: create it, store it in your app struct (or AppState), use it where needed.

### Layer 2: Components

The `tui-dispatch-components` crate provides reusable UI components that follow the patterns above. They're optional - you can write your own components using just Layer 0.

