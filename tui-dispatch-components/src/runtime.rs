use std::io;

use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};
use tui_dispatch_core::runtime::EventBusRouting;
use tui_dispatch_core::{
    Action as ActionTrait, BindingContext, ComponentId, EffectContext, EventBus, EventContext,
    EventRoutingState, Keybindings, NoEffect, RenderContext, Runtime, RuntimeStore, Store,
};

use crate::ComponentHost;

#[doc(hidden)]
/// Bus-routed runtime shape that can be paired with a [`ComponentHost`].
pub type ComponentHostRuntime<S, A, E, Id, Ctx, St = Store<S, A, E>> =
    Runtime<S, A, E, EventBusRouting<S, A, Id, Ctx>, St>;

#[doc(hidden)]
/// Owned pieces returned by [`HostedRuntime::into_parts`].
pub struct HostedRuntimeParts<S, A, E, Id, Ctx, St = Store<S, A, E>>
where
    A: ActionTrait,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: RuntimeStore<S, A, E>,
{
    pub runtime: ComponentHostRuntime<S, A, E, Id, Ctx, St>,
    pub host: ComponentHost<S, A, Id, Ctx>,
}

/// Runtime wrapper that keeps [`ComponentHost`] area synchronization out of app loops.
pub struct HostedRuntime<S, A, E, Id, Ctx, St = Store<S, A, E>>
where
    A: ActionTrait,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    S: EventRoutingState<Id, Ctx>,
    St: RuntimeStore<S, A, E>,
{
    runtime: ComponentHostRuntime<S, A, E, Id, Ctx, St>,
    host: ComponentHost<S, A, Id, Ctx>,
}

/// Extension trait for attaching a [`ComponentHost`] to a bus-routed runtime.
pub trait RuntimeHostExt<H> {
    type Output;

    fn with_component_host(self, host: H) -> Self::Output;
}

impl<S, A, E, Id, Ctx, St> RuntimeHostExt<ComponentHost<S, A, Id, Ctx>>
    for Runtime<S, A, E, EventBusRouting<S, A, Id, Ctx>, St>
where
    S: 'static + EventRoutingState<Id, Ctx>,
    A: ActionTrait,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    St: RuntimeStore<S, A, E>,
{
    type Output = HostedRuntime<S, A, E, Id, Ctx, St>;

    fn with_component_host(self, host: ComponentHost<S, A, Id, Ctx>) -> Self::Output {
        HostedRuntime {
            runtime: self,
            host,
        }
    }
}

impl<S, A, E, Id, Ctx, St> HostedRuntime<S, A, E, Id, Ctx, St>
where
    S: 'static + EventRoutingState<Id, Ctx>,
    A: ActionTrait,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    St: RuntimeStore<S, A, E>,
{
    /// Access the hosted component host.
    pub fn host(&self) -> &ComponentHost<S, A, Id, Ctx> {
        &self.host
    }

    /// Access the hosted component host mutably.
    pub fn host_mut(&mut self) -> &mut ComponentHost<S, A, Id, Ctx> {
        &mut self.host
    }

    /// Access the wrapped runtime.
    pub fn runtime(&self) -> &ComponentHostRuntime<S, A, E, Id, Ctx, St> {
        &self.runtime
    }

    /// Access the wrapped runtime mutably.
    pub fn runtime_mut(&mut self) -> &mut ComponentHostRuntime<S, A, E, Id, Ctx, St> {
        &mut self.runtime
    }

    /// Split the wrapper back into its runtime and host.
    pub fn into_parts(self) -> HostedRuntimeParts<S, A, E, Id, Ctx, St> {
        HostedRuntimeParts {
            runtime: self.runtime,
            host: self.host,
        }
    }

    /// Access the event bus.
    pub fn bus(&self) -> &EventBus<S, A, Id, Ctx> {
        self.runtime.bus()
    }

    /// Access the event bus mutably.
    pub fn bus_mut(&mut self) -> &mut EventBus<S, A, Id, Ctx> {
        self.runtime.bus_mut()
    }

    /// Access keybindings.
    pub fn keybindings(&self) -> &Keybindings<Ctx> {
        self.runtime.keybindings()
    }

    /// Access keybindings mutably.
    pub fn keybindings_mut(&mut self) -> &mut Keybindings<Ctx> {
        self.runtime.keybindings_mut()
    }

    /// Subscribe to action name broadcasts.
    pub fn subscribe_actions(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.runtime.subscribe_actions()
    }

    /// Send an action into the runtime queue.
    pub fn enqueue(&self, action: A) {
        self.runtime.enqueue(action);
    }

    /// Clone the action sender.
    pub fn action_tx(&self) -> tokio::sync::mpsc::UnboundedSender<A> {
        self.runtime.action_tx()
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        self.runtime.state()
    }

    /// Access the task manager.
    #[cfg(feature = "tasks")]
    pub fn tasks(&mut self) -> &mut tui_dispatch_core::TaskManager<A> {
        self.runtime.tasks()
    }

    /// Access subscriptions.
    #[cfg(feature = "subscriptions")]
    pub fn subscriptions(&mut self) -> &mut tui_dispatch_core::Subscriptions<A> {
        self.runtime.subscriptions()
    }
}

impl<S, A, Id, Ctx, St> HostedRuntime<S, A, NoEffect, Id, Ctx, St>
where
    S: 'static + EventRoutingState<Id, Ctx>,
    A: ActionTrait,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    St: RuntimeStore<S, A, NoEffect>,
{
    /// Run the event/action loop and synchronize host-rendered areas after each frame.
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

    /// Run the loop with a post-render hook after host area synchronization.
    pub async fn run_with_hooks<B, FRender, FQuit, FAfter>(
        &mut self,
        terminal: &mut Terminal<B>,
        render: FRender,
        should_quit: FQuit,
        mut after_render: FAfter,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
        FAfter: FnMut(&mut EventBus<S, A, Id, Ctx>, &S),
    {
        let host = self.host.clone();
        self.runtime
            .run_with_hooks(terminal, render, should_quit, move |bus, state| {
                host.sync_areas(bus);
                after_render(bus, state);
            })
            .await
    }
}

impl<S, A, E, Id, Ctx, St> HostedRuntime<S, A, E, Id, Ctx, St>
where
    S: 'static + EventRoutingState<Id, Ctx>,
    A: ActionTrait,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
    St: RuntimeStore<S, A, E>,
{
    /// Run the event/action loop with effects and host area synchronization.
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

    /// Run the loop with effects and a post-render hook after host area synchronization.
    pub async fn run_with_effect_hooks<B, FRender, FQuit, FEffect, FAfter>(
        &mut self,
        terminal: &mut Terminal<B>,
        render: FRender,
        should_quit: FQuit,
        handle_effect: FEffect,
        mut after_render: FAfter,
    ) -> io::Result<()>
    where
        B: Backend,
        FRender: FnMut(&mut Frame, Rect, &S, RenderContext, &mut EventContext<Id>),
        FQuit: FnMut(&A) -> bool,
        FEffect: FnMut(E, &mut EffectContext<A>),
        FAfter: FnMut(&mut EventBus<S, A, Id, Ctx>, &S),
    {
        let host = self.host.clone();
        self.runtime
            .run_with_effect_hooks(
                terminal,
                render,
                should_quit,
                handle_effect,
                move |bus, state| {
                    host.sync_areas(bus);
                    after_render(bus, state);
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;
    use tui_dispatch_core::{
        Action, DefaultBindingContext, ReducerResult, Runtime, SimpleEventBus,
    };

    use super::*;
    use crate::{ComponentDebugState, InteractiveComponent};

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestAction {
        Quit,
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            "quit"
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum TestId {
        Main,
    }

    impl ComponentId for TestId {
        fn name(&self) -> &'static str {
            "main"
        }
    }

    #[derive(Default)]
    struct TestState {
        focused: Option<TestId>,
    }

    impl EventRoutingState<TestId, DefaultBindingContext> for TestState {
        fn focused(&self) -> Option<TestId> {
            self.focused
        }

        fn modal(&self) -> Option<TestId> {
            None
        }

        fn binding_context(&self, _id: TestId) -> DefaultBindingContext {
            DefaultBindingContext
        }

        fn default_context(&self) -> DefaultBindingContext {
            DefaultBindingContext
        }
    }

    struct TestComponent;

    impl ComponentDebugState for TestComponent {}

    impl InteractiveComponent<TestAction> for TestComponent {
        type Props<'a> = ();

        fn render(&mut self, frame: &mut Frame, area: Rect, _props: Self::Props<'_>) {
            frame.render_widget(Paragraph::new("hosted"), area);
        }
    }

    fn reducer(_state: &mut TestState, _action: TestAction) -> ReducerResult {
        ReducerResult::unchanged()
    }

    fn unit_props(_state: &TestState) {}

    #[tokio::test]
    async fn hosted_runtime_syncs_component_areas_after_render() {
        let host = ComponentHost::<TestState, TestAction, TestId, DefaultBindingContext>::new();
        let mounted = host.mount::<TestComponent, _>(|| TestComponent, unit_props);

        let mut bus = SimpleEventBus::<TestState, TestAction, TestId>::new();
        host.bind(&mut bus, TestId::Main, mounted);

        let mut runtime = Runtime::new(
            TestState {
                focused: Some(TestId::Main),
            },
            reducer,
        )
        .with_event_bus(bus, Keybindings::new())
        .with_component_host(host.clone());

        runtime.enqueue(TestAction::Quit);

        let backend = TestBackend::new(8, 1);
        let mut terminal = Terminal::new(backend).expect("test backend should initialize");

        runtime
            .run(
                &mut terminal,
                |frame, area, state, _render_ctx, _event_ctx| {
                    host.render(mounted, frame, area, state);
                },
                |action| matches!(action, TestAction::Quit),
            )
            .await
            .expect("runtime should exit on queued quit action");

        assert_eq!(
            runtime.bus().context().component_areas.get(&TestId::Main),
            Some(&Rect::new(0, 0, 8, 1))
        );
    }
}
