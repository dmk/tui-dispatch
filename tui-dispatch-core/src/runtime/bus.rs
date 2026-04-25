//! Event-bus routing mode for [`Runtime`](super::Runtime).

use std::io;

use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};

use crate::bus::{EventBus, EventRoutingState};
use crate::event::{ComponentId, EventContext};
use crate::keybindings::Keybindings;
use crate::store::NoEffect;
use crate::{Action, BindingContext};

use super::core::{draw_frame, EffectContext, RenderContext, Runtime, RuntimeStore};

/// Runtime routing mode that routes raw events through an [`EventBus`].
#[doc(hidden)]
pub struct EventBusRouting<S, A, Id, Ctx>
where
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
{
    pub(crate) bus: EventBus<S, A, Id, Ctx>,
    pub(crate) keybindings: Keybindings<Ctx>,
}

impl<S, A, E, Routing, St> Runtime<S, A, E, Routing, St>
where
    S: 'static,
    A: Action,
    St: RuntimeStore<S, A, E>,
{
    /// Pair this runtime with an [`EventBus`] + [`Keybindings`] so raw events
    /// are routed through the bus before actions are dispatched.
    pub fn with_event_bus<Id, Ctx>(
        self,
        bus: EventBus<S, A, Id, Ctx>,
        keybindings: Keybindings<Ctx>,
    ) -> Runtime<S, A, E, EventBusRouting<S, A, Id, Ctx>, St>
    where
        Id: ComponentId + 'static,
        Ctx: BindingContext + 'static,
        S: EventRoutingState<Id, Ctx>,
    {
        Runtime {
            store: self.store,
            shell: self.shell,
            routing: EventBusRouting { bus, keybindings },
            #[cfg(feature = "tasks")]
            tasks: self.tasks,
            #[cfg(feature = "subscriptions")]
            subscriptions: self.subscriptions,
            action_broadcast: self.action_broadcast,
            _effect: std::marker::PhantomData,
        }
    }
}

impl<S, A, E, Id, Ctx, St> Runtime<S, A, E, EventBusRouting<S, A, Id, Ctx>, St>
where
    S: 'static,
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: RuntimeStore<S, A, E>,
{
    /// Access the event bus.
    pub fn bus(&self) -> &EventBus<S, A, Id, Ctx> {
        &self.routing.bus
    }

    /// Access the event bus mutably.
    pub fn bus_mut(&mut self) -> &mut EventBus<S, A, Id, Ctx> {
        &mut self.routing.bus
    }

    /// Access the keybindings.
    pub fn keybindings(&self) -> &Keybindings<Ctx> {
        &self.routing.keybindings
    }

    /// Access the keybindings mutably.
    pub fn keybindings_mut(&mut self) -> &mut Keybindings<Ctx> {
        &mut self.routing.keybindings
    }
}

impl<S, A, Id, Ctx, St> Runtime<S, A, NoEffect, EventBusRouting<S, A, Id, Ctx>, St>
where
    S: 'static,
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: RuntimeStore<S, A, NoEffect>,
{
    /// Run the event/action loop until quit, routing raw events through the
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
    /// frame is drawn.
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
        let (mut event_rx, cancel_token) = self.shell.spawn_poller();

        loop {
            if self.shell.should_render {
                let bus = &mut self.routing.bus;
                draw_frame(
                    &mut self.shell,
                    self.store.state(),
                    terminal,
                    |f, area, s, ctx| render(f, area, s, ctx, bus.context_mut()),
                )?;
                after_render(&mut self.routing.bus, self.store.state());
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let bus = &mut self.routing.bus;
                    let kb = &self.routing.keybindings;
                    self.shell.process_event(
                        raw_event,
                        self.store.state(),
                        |event, state| bus.handle_event(event, state, kb),
                    );
                }

                Some(action) = self.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.shell.debug_log_action(&action);
                    if self.dispatch_action(action) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        self.cleanup(cancel_token);
        Ok(())
    }
}

impl<S, A, E, Id, Ctx, St> Runtime<S, A, E, EventBusRouting<S, A, Id, Ctx>, St>
where
    S: 'static,
    A: Action,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: RuntimeStore<S, A, E>,
{
    /// Run the event/action loop until quit, routing raw events through the
    /// bus + keybindings and handling emitted effects at the run boundary.
    pub async fn run_with_effects<B, FRender, FQuit, FEffect>(
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
        self.run_with_effect_hooks(terminal, render, should_quit, handle_effect, |_, _| {})
            .await
    }

    /// Run the event/action loop with effects and a post-render hook invoked
    /// after each frame is drawn.
    pub async fn run_with_effect_hooks<B, FRender, FQuit, FEffect, FAfter>(
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
        let (mut event_rx, cancel_token) = self.shell.spawn_poller();

        loop {
            if self.shell.should_render {
                let bus = &mut self.routing.bus;
                draw_frame(
                    &mut self.shell,
                    self.store.state(),
                    terminal,
                    |f, area, s, ctx| render(f, area, s, ctx, bus.context_mut()),
                )?;
                after_render(&mut self.routing.bus, self.store.state());
            }

            tokio::select! {
                Some(raw_event) = event_rx.recv() => {
                    let bus = &mut self.routing.bus;
                    let kb = &self.routing.keybindings;
                    self.shell.process_event(
                        raw_event,
                        self.store.state(),
                        |event, state| bus.handle_event(event, state, kb),
                    );
                }

                Some(action) = self.shell.action_rx.recv() => {
                    if should_quit(&action) {
                        break;
                    }
                    self.shell.debug_log_action(&action);
                    if self.dispatch_and_handle_effects(action, &mut handle_effect) {
                        break;
                    }
                }

                else => { break; }
            }
        }

        self.cleanup(cancel_token);
        Ok(())
    }
}
