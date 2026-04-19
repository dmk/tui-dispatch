//! Layer 1 bus-composition wrappers.
//!
//! `BusDispatchRuntime` and `BusEffectRuntime` pair a base runtime with an
//! [`EventBus`] and [`Keybindings`], reached via `with_event_bus(...)` on the
//! base runtime. Their `run_with_hooks` method exposes a post-render hook so
//! higher-level wrappers (e.g. a future `HostedDispatchRuntime`) can plug in
//! without touching core.

use std::io;

use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};
use tokio::sync::mpsc;

use crate::bus::{EventBus, EventRoutingState};
use crate::event::{ComponentId, EventContext};
use crate::keybindings::Keybindings;
use crate::store::Store;
use crate::{Action, BindingContext};

use super::core::{
    draw_frame, DispatchRuntime, DispatchStore, EffectContext, EffectDispatchStore, EffectRuntime,
    RenderContext,
};

use crate::effect::EffectStore;

#[cfg(feature = "subscriptions")]
use crate::subscriptions::Subscriptions;
#[cfg(feature = "tasks")]
use crate::tasks::TaskManager;

// ---------------------------------------------------------------------------
// BusDispatchRuntime — DispatchRuntime + EventBus + Keybindings
// ---------------------------------------------------------------------------

/// A [`DispatchRuntime`] paired with an [`EventBus`] and [`Keybindings`].
///
/// Created via [`DispatchRuntime::with_event_bus`]. Drives the same
/// event/action/render loop as the base runtime, but routes raw events
/// through the bus (focus, keybindings, subscriptions) before dispatching.
pub struct BusDispatchRuntime<S, A, Id, Ctx, St = Store<S, A>>
where
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: DispatchStore<S, A>,
{
    inner: DispatchRuntime<S, A, St>,
    bus: EventBus<S, A, Id, Ctx>,
    keybindings: Keybindings<Ctx>,
}

impl<S, A, Id, Ctx, St> BusDispatchRuntime<S, A, Id, Ctx, St>
where
    S: 'static,
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: DispatchStore<S, A>,
{
    /// Access the event bus.
    pub fn bus(&self) -> &EventBus<S, A, Id, Ctx> {
        &self.bus
    }

    /// Access the event bus mutably (e.g. to register components before `run`).
    pub fn bus_mut(&mut self) -> &mut EventBus<S, A, Id, Ctx> {
        &mut self.bus
    }

    /// Access the keybindings.
    pub fn keybindings(&self) -> &Keybindings<Ctx> {
        &self.keybindings
    }

    /// Access the keybindings mutably.
    pub fn keybindings_mut(&mut self) -> &mut Keybindings<Ctx> {
        &mut self.keybindings
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        self.inner.state()
    }

    /// Send an action into the runtime queue.
    pub fn enqueue(&self, action: A) {
        self.inner.enqueue(action);
    }

    /// Clone the action sender.
    pub fn action_tx(&self) -> mpsc::UnboundedSender<A> {
        self.inner.action_tx()
    }

    /// Access the wrapped base runtime.
    pub fn inner(&self) -> &DispatchRuntime<S, A, St> {
        &self.inner
    }

    /// Access the wrapped base runtime mutably.
    pub fn inner_mut(&mut self) -> &mut DispatchRuntime<S, A, St> {
        &mut self.inner
    }

    /// Run the event/action loop until quit. Routes raw events through the
    /// bus + keybindings.
    pub async fn run<B, FRender, FQuit>(
        &mut self,
        terminal: &mut Terminal<B>,
        render: FRender,
        should_quit: FQuit,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
    {
        self.run_with_hooks(terminal, render, should_quit, |_, _| {})
            .await
    }

    /// Run the event/action loop with a post-render hook invoked after each
    /// frame is drawn. The hook gets `&mut EventBus` and `&S` so wrappers can
    /// e.g. sync component areas before the next event is processed.
    pub async fn run_with_hooks<B, FRender, FQuit, FAfter>(
        &mut self,
        terminal: &mut Terminal<B>,
        mut render: FRender,
        mut should_quit: FQuit,
        mut after_render: FAfter,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
        FAfter: FnMut(&mut EventBus<S, A, Id, Ctx>, &S),
    {
        let (mut event_rx, cancel_token) = self.inner.shell.spawn_poller();

        loop {
            if self.inner.shell.should_render {
                let bus = &mut self.bus;
                draw_frame(
                    &mut self.inner.shell,
                    self.inner.store.state(),
                    terminal,
                    |f, area, s, ctx| render(f, area, s, ctx, bus.context_mut()),
                )?;
                after_render(&mut self.bus, self.inner.store.state());
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let bus = &mut self.bus;
                    let kb = &self.keybindings;
                    self.inner.shell.process_event(
                        raw_event,
                        self.inner.store.state(),
                        |event, state| bus.handle_event(event, state, kb),
                    );
                }

                Some(action) = self.inner.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.inner.shell.debug_log_action(&action);
                    if self.inner.dispatch_action(action) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        cancel_token.cancel();
        Ok(())
    }
}

impl<S: 'static, A: Action, St: DispatchStore<S, A>> DispatchRuntime<S, A, St> {
    /// Pair this runtime with an [`EventBus`] + [`Keybindings`] to get a
    /// [`BusDispatchRuntime`] that routes events through the bus.
    pub fn with_event_bus<Id, Ctx>(
        self,
        bus: EventBus<S, A, Id, Ctx>,
        keybindings: Keybindings<Ctx>,
    ) -> BusDispatchRuntime<S, A, Id, Ctx, St>
    where
        Id: ComponentId + 'static,
        Ctx: BindingContext + 'static,
        S: EventRoutingState<Id, Ctx>,
    {
        BusDispatchRuntime {
            inner: self,
            bus,
            keybindings,
        }
    }
}

// ---------------------------------------------------------------------------
// BusEffectRuntime — EffectRuntime + EventBus + Keybindings
// ---------------------------------------------------------------------------

/// An [`EffectRuntime`] paired with an [`EventBus`] and [`Keybindings`].
///
/// Created via [`EffectRuntime::with_event_bus`]. Drives the same
/// event/action/render loop as the base runtime, but routes raw events
/// through the bus (focus, keybindings, subscriptions) before dispatching.
pub struct BusEffectRuntime<S, A, E, Id, Ctx, St = EffectStore<S, A, E>>
where
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: EffectDispatchStore<S, A, E>,
{
    inner: EffectRuntime<S, A, E, St>,
    bus: EventBus<S, A, Id, Ctx>,
    keybindings: Keybindings<Ctx>,
}

impl<S, A, E, Id, Ctx, St> BusEffectRuntime<S, A, E, Id, Ctx, St>
where
    S: 'static,
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: EffectDispatchStore<S, A, E>,
{
    /// Access the event bus.
    pub fn bus(&self) -> &EventBus<S, A, Id, Ctx> {
        &self.bus
    }

    /// Access the event bus mutably (e.g. to register components before `run`).
    pub fn bus_mut(&mut self) -> &mut EventBus<S, A, Id, Ctx> {
        &mut self.bus
    }

    /// Access the keybindings.
    pub fn keybindings(&self) -> &Keybindings<Ctx> {
        &self.keybindings
    }

    /// Access the keybindings mutably.
    pub fn keybindings_mut(&mut self) -> &mut Keybindings<Ctx> {
        &mut self.keybindings
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        self.inner.state()
    }

    /// Send an action into the runtime queue.
    pub fn enqueue(&self, action: A) {
        self.inner.enqueue(action);
    }

    /// Clone the action sender.
    pub fn action_tx(&self) -> mpsc::UnboundedSender<A> {
        self.inner.action_tx()
    }

    /// Subscribe to action name broadcasts.
    pub fn subscribe_actions(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.inner.subscribe_actions()
    }

    /// Access the task manager.
    #[cfg(feature = "tasks")]
    pub fn tasks(&mut self) -> &mut TaskManager<A> {
        self.inner.tasks()
    }

    /// Access subscriptions.
    #[cfg(feature = "subscriptions")]
    pub fn subscriptions(&mut self) -> &mut Subscriptions<A> {
        self.inner.subscriptions()
    }

    /// Access the wrapped base runtime.
    pub fn inner(&self) -> &EffectRuntime<S, A, E, St> {
        &self.inner
    }

    /// Access the wrapped base runtime mutably.
    pub fn inner_mut(&mut self) -> &mut EffectRuntime<S, A, E, St> {
        &mut self.inner
    }

    /// Run the event/action loop until quit. Routes raw events through the
    /// bus + keybindings.
    pub async fn run<B, FRender, FQuit, FEffect>(
        &mut self,
        terminal: &mut Terminal<B>,
        render: FRender,
        should_quit: FQuit,
        handle_effect: FEffect,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
        FEffect: FnMut(E, &mut EffectContext<A>),
    {
        self.run_with_hooks(terminal, render, should_quit, handle_effect, |_, _| {})
            .await
    }

    /// Run the event/action loop with a post-render hook invoked after each
    /// frame is drawn.
    pub async fn run_with_hooks<B, FRender, FQuit, FEffect, FAfter>(
        &mut self,
        terminal: &mut Terminal<B>,
        mut render: FRender,
        mut should_quit: FQuit,
        mut handle_effect: FEffect,
        mut after_render: FAfter,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
        FEffect: FnMut(E, &mut EffectContext<A>),
        FAfter: FnMut(&mut EventBus<S, A, Id, Ctx>, &S),
    {
        let (mut event_rx, cancel_token) = self.inner.shell.spawn_poller();

        loop {
            if self.inner.shell.should_render {
                let bus = &mut self.bus;
                draw_frame(
                    &mut self.inner.shell,
                    self.inner.store.state(),
                    terminal,
                    |f, area, s, ctx| render(f, area, s, ctx, bus.context_mut()),
                )?;
                after_render(&mut self.bus, self.inner.store.state());
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let bus = &mut self.bus;
                    let kb = &self.keybindings;
                    self.inner.shell.process_event(
                        raw_event,
                        self.inner.store.state(),
                        |event, state| bus.handle_event(event, state, kb),
                    );
                }

                Some(action) = self.inner.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.inner.shell.debug_log_action(&action);
                    if self.inner.dispatch_and_handle_effects(action, &mut handle_effect) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        self.inner.cleanup(cancel_token);
        Ok(())
    }
}

impl<S: 'static, A: Action, E, St: EffectDispatchStore<S, A, E>> EffectRuntime<S, A, E, St> {
    /// Pair this runtime with an [`EventBus`] + [`Keybindings`] to get a
    /// [`BusEffectRuntime`] that routes events through the bus.
    pub fn with_event_bus<Id, Ctx>(
        self,
        bus: EventBus<S, A, Id, Ctx>,
        keybindings: Keybindings<Ctx>,
    ) -> BusEffectRuntime<S, A, E, Id, Ctx, St>
    where
        Id: ComponentId + 'static,
        Ctx: BindingContext + 'static,
        S: EventRoutingState<Id, Ctx>,
    {
        BusEffectRuntime {
            inner: self,
            bus,
            keybindings,
        }
    }
}
