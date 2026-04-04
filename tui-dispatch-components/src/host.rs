use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use crossterm::event::{KeyEvent, KeyModifiers, MouseEvent};
use ratatui::{layout::Rect, Frame};
use tui_dispatch_core::{
    Action as ActionTrait, BindingContext, ComponentId, DefaultBindingContext, EventBus, EventKind,
    HandlerResponse, RoutedEvent,
};

/// Routed input delivered to interactive components.
#[derive(Debug, Clone)]
pub enum ComponentInput<'a, Ctx> {
    Command {
        name: &'a str,
        ctx: Ctx,
    },
    Key(KeyEvent),
    Mouse(MouseEvent),
    Scroll {
        column: u16,
        row: u16,
        delta: isize,
        modifiers: KeyModifiers,
    },
    Resize(u16, u16),
    Tick,
}

impl<'a, Id, Ctx> From<RoutedEvent<'a, Id, Ctx>> for ComponentInput<'a, Ctx>
where
    Id: ComponentId,
    Ctx: BindingContext,
{
    fn from(event: RoutedEvent<'a, Id, Ctx>) -> Self {
        if let Some(name) = event.command {
            return Self::Command {
                name,
                ctx: event.binding_ctx,
            };
        }

        match event.kind {
            EventKind::Key(key) => Self::Key(key),
            EventKind::Mouse(mouse) => Self::Mouse(mouse),
            EventKind::Scroll {
                column,
                row,
                delta,
                modifiers,
            } => Self::Scroll {
                column,
                row,
                delta,
                modifiers,
            },
            EventKind::Resize(width, height) => Self::Resize(width, height),
            EventKind::Tick => Self::Tick,
        }
    }
}

/// Optional component-local state inspection for debug tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDebugEntry {
    pub key: String,
    pub value: String,
}

impl ComponentDebugEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Optional debug surface for interactive components.
pub trait ComponentDebugState {
    fn debug_state(&self) -> Vec<ComponentDebugEntry> {
        Vec::new()
    }
}

/// Richer component contract used by [`ComponentHost`].
pub trait InteractiveComponent<A, Ctx = DefaultBindingContext>: ComponentDebugState {
    type Props<'a>
    where
        Self: 'a;

    #[allow(unused_variables)]
    fn update(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: Self::Props<'_>,
    ) -> HandlerResponse<A> {
        HandlerResponse::ignored()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>);
}

/// Factory that builds component props from store state.
pub trait PropsFactory<S, C, A, Ctx>: 'static
where
    C: InteractiveComponent<A, Ctx> + 'static,
{
    fn props<'a>(&self, state: &'a S) -> C::Props<'a>;
}

impl<S, C, A, Ctx, F> PropsFactory<S, C, A, Ctx> for F
where
    C: InteractiveComponent<A, Ctx> + 'static,
    F: for<'a> Fn(&'a S) -> C::Props<'a> + 'static,
{
    fn props<'a>(&self, state: &'a S) -> C::Props<'a> {
        (self)(state)
    }
}

/// Typed handle to a mounted component.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Mounted<C> {
    raw: u32,
    _marker: PhantomData<fn() -> C>,
}

impl<C> Mounted<C> {
    fn new(raw: u32) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }
}

impl<C> Copy for Mounted<C> {}

impl<C> Clone for Mounted<C> {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostLifecycleError<Id> {
    NotMounted(u32),
    StillBound(Id),
    TypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedComponentInfo<Id> {
    pub raw: u32,
    pub type_name: &'static str,
    pub bound_id: Option<Id>,
    pub last_area: Option<Rect>,
    pub debug_state: Vec<ComponentDebugEntry>,
}

pub struct ComponentHost<S, A, Id, Ctx> {
    inner: Rc<RefCell<ComponentHostInner<S, A, Id, Ctx>>>,
}

impl<S, A, Id, Ctx> Clone for ComponentHost<S, A, Id, Ctx> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<S, A, Id, Ctx> Default for ComponentHost<S, A, Id, Ctx>
where
    S: 'static,
    A: 'static,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

trait ErasedMounted<S, A, Id, Ctx> {
    fn type_id(&self) -> TypeId;
    fn type_name(&self) -> &'static str;
    fn binding(&self) -> Option<Id>;
    fn set_binding(&mut self, id: Option<Id>);
    fn last_area(&self) -> Option<Rect>;
    fn render_epoch(&self) -> u64;
    fn update(&mut self, input: ComponentInput<'_, Ctx>, state: &S) -> HandlerResponse<A>;
    fn render(&mut self, frame: &mut Frame, area: Rect, state: &S, frame_epoch: u64);
    fn reset_local_state(&mut self);
    fn debug_state(&self) -> Vec<ComponentDebugEntry>;
}

struct MountedEntry<S, A, Id, Ctx, C>
where
    C: InteractiveComponent<A, Ctx> + 'static,
{
    component: C,
    factory: Box<dyn Fn() -> C>,
    props_factory: Box<dyn PropsFactory<S, C, A, Ctx>>,
    bound_id: Option<Id>,
    last_area: Option<Rect>,
    last_render_epoch: u64,
}

impl<S, A, Id, Ctx, C> ErasedMounted<S, A, Id, Ctx> for MountedEntry<S, A, Id, Ctx, C>
where
    S: 'static,
    A: 'static,
    Ctx: 'static,
    C: InteractiveComponent<A, Ctx> + 'static,
    Id: Copy,
{
    fn type_id(&self) -> TypeId {
        TypeId::of::<C>()
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<C>()
    }

    fn binding(&self) -> Option<Id> {
        self.bound_id
    }

    fn set_binding(&mut self, id: Option<Id>) {
        self.bound_id = id;
    }

    fn last_area(&self) -> Option<Rect> {
        self.last_area
    }

    fn render_epoch(&self) -> u64 {
        self.last_render_epoch
    }

    fn update(&mut self, input: ComponentInput<'_, Ctx>, state: &S) -> HandlerResponse<A> {
        let props = self.props_factory.props(state);
        self.component.update(input, props)
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, state: &S, frame_epoch: u64) {
        self.last_area = Some(area);
        self.last_render_epoch = frame_epoch;
        let props = self.props_factory.props(state);
        self.component.render(frame, area, props);
    }

    fn reset_local_state(&mut self) {
        self.component = (self.factory)();
    }

    fn debug_state(&self) -> Vec<ComponentDebugEntry> {
        self.component.debug_state()
    }
}

struct ComponentHostInner<S, A, Id, Ctx> {
    next_raw: u32,
    frame_epoch: u64,
    mounted: HashMap<u32, Box<dyn ErasedMounted<S, A, Id, Ctx>>>,
    bindings: HashMap<Id, u32>,
}

impl<S, A, Id, Ctx> ComponentHostInner<S, A, Id, Ctx>
where
    Id: ComponentId,
    Ctx: BindingContext,
{
    fn new() -> Self {
        Self {
            next_raw: 1,
            frame_epoch: 1,
            mounted: HashMap::new(),
            bindings: HashMap::new(),
        }
    }
}

impl<S, A, Id, Ctx> ComponentHost<S, A, Id, Ctx>
where
    S: 'static,
    A: 'static,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
{
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ComponentHostInner::new())),
        }
    }

    pub fn mount<C, P>(&self, factory: impl Fn() -> C + 'static, props: P) -> Mounted<C>
    where
        C: InteractiveComponent<A, Ctx> + 'static,
        P: PropsFactory<S, C, A, Ctx> + 'static,
    {
        let mut inner = self.inner.borrow_mut();
        let raw = inner.next_raw;
        inner.next_raw = inner.next_raw.saturating_add(1);
        let component = factory();
        inner.mounted.insert(
            raw,
            Box::new(MountedEntry {
                component,
                factory: Box::new(factory),
                props_factory: Box::new(props),
                bound_id: None,
                last_area: None,
                last_render_epoch: 0,
            }),
        );
        Mounted::new(raw)
    }

    pub fn unmount<C>(&self, mounted: Mounted<C>) -> Result<(), HostLifecycleError<Id>>
    where
        C: InteractiveComponent<A, Ctx> + 'static,
    {
        let mut inner = self.inner.borrow_mut();
        inner.ensure_type::<C>(mounted.raw)?;
        let entry = inner
            .mounted
            .get(&mounted.raw)
            .ok_or(HostLifecycleError::NotMounted(mounted.raw))?;

        if let Some(id) = entry.binding() {
            return Err(HostLifecycleError::StillBound(id));
        }

        inner.mounted.remove(&mounted.raw);
        Ok(())
    }

    pub fn update<C>(
        &self,
        mounted: Mounted<C>,
        input: ComponentInput<'_, Ctx>,
        state: &S,
    ) -> HandlerResponse<A>
    where
        C: InteractiveComponent<A, Ctx> + 'static,
    {
        let mut inner = self.inner.borrow_mut();
        inner.expect_type::<C>(mounted.raw);
        inner
            .mounted
            .get_mut(&mounted.raw)
            .expect("component handle points to an unmounted component")
            .update(input, state)
    }

    pub fn render<C>(&self, mounted: Mounted<C>, frame: &mut Frame, area: Rect, state: &S)
    where
        C: InteractiveComponent<A, Ctx> + 'static,
    {
        let mut inner = self.inner.borrow_mut();
        inner.expect_type::<C>(mounted.raw);
        let frame_epoch = inner.frame_epoch;
        inner
            .mounted
            .get_mut(&mounted.raw)
            .expect("component handle points to an unmounted component")
            .render(frame, area, state, frame_epoch);
    }

    pub fn reset_local_state(&self) {
        let mut inner = self.inner.borrow_mut();
        for entry in inner.mounted.values_mut() {
            entry.reset_local_state();
        }
    }

    pub fn mounted_components(&self) -> Vec<MountedComponentInfo<Id>> {
        let inner = self.inner.borrow();
        let mut info: Vec<_> = inner
            .mounted
            .iter()
            .map(|(raw, entry)| MountedComponentInfo {
                raw: *raw,
                type_name: entry.type_name(),
                bound_id: entry.binding(),
                last_area: entry.last_area(),
                debug_state: entry.debug_state(),
            })
            .collect();
        info.sort_by_key(|entry| entry.raw);
        info
    }
}

impl<S, A, Id, Ctx> ComponentHost<S, A, Id, Ctx>
where
    S: 'static,
    A: 'static + ActionTrait,
    Id: ComponentId + 'static,
    Ctx: BindingContext + 'static,
{
    pub fn bind<C>(&self, bus: &mut EventBus<S, A, Id, Ctx>, id: Id, mounted: Mounted<C>)
    where
        C: InteractiveComponent<A, Ctx> + 'static,
    {
        let (replaced_route, previous_binding) = {
            let mut inner = self.inner.borrow_mut();
            inner.expect_type::<C>(mounted.raw);

            let previous_binding = inner
                .mounted
                .get(&mounted.raw)
                .expect("component handle points to an unmounted component")
                .binding();
            let replaced_route = inner.bindings.get(&id).copied();

            if let Some(previous_raw) = replaced_route {
                if previous_raw != mounted.raw {
                    if let Some(entry) = inner.mounted.get_mut(&previous_raw) {
                        entry.set_binding(None);
                    }
                }
            }

            if let Some(previous_id) = previous_binding {
                if previous_id != id {
                    inner.bindings.remove(&previous_id);
                }
            }

            inner.bindings.insert(id, mounted.raw);
            if let Some(entry) = inner.mounted.get_mut(&mounted.raw) {
                entry.set_binding(Some(id));
            }

            (replaced_route, previous_binding)
        };

        if replaced_route.is_some() {
            bus.unregister(id);
        }
        if let Some(previous_id) = previous_binding {
            if previous_id != id {
                bus.unregister(previous_id);
                bus.context_mut().remove_component_area(previous_id);
            }
        }

        let host = self.clone();
        bus.register(id, move |event, state| {
            host.update(mounted, event.into(), state)
        });
    }

    pub fn unbind(&self, bus: &mut EventBus<S, A, Id, Ctx>, id: Id) {
        let mut inner = self.inner.borrow_mut();
        if let Some(raw) = inner.bindings.remove(&id) {
            if let Some(entry) = inner.mounted.get_mut(&raw) {
                entry.set_binding(None);
            }
        }
        bus.unregister(id);
        bus.context_mut().remove_component_area(id);
    }

    pub fn sync_areas(&self, bus: &mut EventBus<S, A, Id, Ctx>) {
        let mut areas = Vec::new();
        {
            let mut inner = self.inner.borrow_mut();
            let frame_epoch = inner.frame_epoch;
            for entry in inner.mounted.values() {
                if let Some(id) = entry.binding() {
                    let area = if entry.render_epoch() == frame_epoch {
                        entry.last_area()
                    } else {
                        None
                    };
                    areas.push((id, area));
                }
            }
            inner.frame_epoch = inner.frame_epoch.saturating_add(1);
        }

        let context = bus.context_mut();
        for (id, area) in areas {
            if let Some(area) = area {
                context.set_component_area(id, area);
            } else {
                context.remove_component_area(id);
            }
        }
    }
}

impl<S, A, Id, Ctx> ComponentHostInner<S, A, Id, Ctx>
where
    Id: ComponentId,
    Ctx: BindingContext,
{
    fn ensure_type<C>(&self, raw: u32) -> Result<(), HostLifecycleError<Id>>
    where
        C: 'static,
    {
        let Some(entry) = self.mounted.get(&raw) else {
            return Err(HostLifecycleError::NotMounted(raw));
        };

        if entry.type_id() == TypeId::of::<C>() {
            Ok(())
        } else {
            Err(HostLifecycleError::TypeMismatch {
                expected: std::any::type_name::<C>(),
                actual: entry.type_name(),
            })
        }
    }

    fn expect_type<C>(&self, raw: u32)
    where
        C: 'static,
    {
        if let Err(err) = self.ensure_type::<C>(raw) {
            panic!("invalid mounted component handle: {err:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyEventState, MouseButton, MouseEventKind,
    };
    use tui_dispatch_core::testing::{key, RenderHarness};
    use tui_dispatch_core::{
        Action, BindingContext, EventRoutingState, EventType, Keybindings, RouteTarget,
    };

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestAction {
        Emit(String),
    }

    impl Action for TestAction {
        fn name(&self) -> &'static str {
            "emit"
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum TestId {
        A,
    }

    impl ComponentId for TestId {
        fn name(&self) -> &'static str {
            match self {
                Self::A => "a",
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    enum TestCtx {
        #[default]
        Default,
        Nav,
    }

    impl BindingContext for TestCtx {
        fn name(&self) -> &'static str {
            match self {
                Self::Default => "default",
                Self::Nav => "nav",
            }
        }

        fn from_name(name: &str) -> Option<Self> {
            match name {
                "default" => Some(Self::Default),
                "nav" => Some(Self::Nav),
                _ => None,
            }
        }

        fn all() -> &'static [Self] {
            &[Self::Default, Self::Nav]
        }
    }

    struct TestState {
        focused: Option<TestId>,
        context: TestCtx,
        label: String,
    }

    fn label_props(state: &TestState) -> &str {
        state.label.as_str()
    }

    impl EventRoutingState<TestId, TestCtx> for TestState {
        fn focused(&self) -> Option<TestId> {
            self.focused
        }

        fn modal(&self) -> Option<TestId> {
            None
        }

        fn binding_context(&self, _id: TestId) -> TestCtx {
            self.context
        }

        fn default_context(&self) -> TestCtx {
            TestCtx::Default
        }
    }

    struct EchoComponent {
        resets: Rc<Cell<usize>>,
    }

    impl ComponentDebugState for EchoComponent {
        fn debug_state(&self) -> Vec<ComponentDebugEntry> {
            vec![ComponentDebugEntry::new(
                "resets",
                self.resets.get().to_string(),
            )]
        }
    }

    impl InteractiveComponent<TestAction, TestCtx> for EchoComponent {
        type Props<'a> = &'a str;

        fn update(
            &mut self,
            input: ComponentInput<'_, TestCtx>,
            props: Self::Props<'_>,
        ) -> HandlerResponse<TestAction> {
            match input {
                ComponentInput::Command { name, .. } => {
                    HandlerResponse::action(TestAction::Emit(name.to_string()))
                }
                ComponentInput::Key(key) if key.code == KeyCode::Char('x') => {
                    HandlerResponse::action(TestAction::Emit("raw".into()))
                }
                ComponentInput::Mouse(mouse)
                    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) =>
                {
                    HandlerResponse::action(TestAction::Emit("mouse".into())).with_render()
                }
                ComponentInput::Scroll { .. } => {
                    HandlerResponse::action(TestAction::Emit(props.to_string())).with_render()
                }
                ComponentInput::Resize(..) => HandlerResponse::ignored().with_render(),
                ComponentInput::Tick => HandlerResponse::action(TestAction::Emit("tick".into())),
                _ => HandlerResponse::ignored(),
            }
        }

        fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
            use ratatui::widgets::Paragraph;
            frame.render_widget(Paragraph::new(props.to_string()), area);
        }
    }

    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn mounted_host_reuses_props_and_resets_local_state() {
        let host = ComponentHost::<TestState, TestAction, TestId, TestCtx>::new();
        let resets = Rc::new(Cell::new(0));
        let mounted: Mounted<EchoComponent> = host.mount::<EchoComponent, _>(
            {
                let resets = Rc::clone(&resets);
                move || {
                    resets.set(resets.get() + 1);
                    EchoComponent {
                        resets: Rc::clone(&resets),
                    }
                }
            },
            label_props,
        );

        let mut harness = RenderHarness::new(12, 1);
        let state = TestState {
            focused: Some(TestId::A),
            context: TestCtx::Default,
            label: "hello".into(),
        };
        let output = harness
            .render_to_string_plain(|frame| host.render(mounted, frame, frame.area(), &state));
        assert!(output.contains("hello"));

        let response = host.update(
            mounted,
            ComponentInput::Scroll {
                column: 0,
                row: 0,
                delta: 1,
                modifiers: KeyModifiers::NONE,
            },
            &state,
        );
        assert_eq!(response.actions, vec![TestAction::Emit("hello".into())]);
        assert!(response.needs_render);

        host.reset_local_state();
        let info = host.mounted_components();
        assert_eq!(
            info[0].debug_state[0],
            ComponentDebugEntry::new("resets", "2")
        );
    }

    #[test]
    fn bind_routes_commands_and_syncs_areas() {
        let host = ComponentHost::<TestState, TestAction, TestId, TestCtx>::new();
        let mounted: Mounted<EchoComponent> = host.mount::<EchoComponent, _>(
            || EchoComponent {
                resets: Rc::new(Cell::new(1)),
            },
            label_props,
        );

        let mut bus = EventBus::<TestState, TestAction, TestId, TestCtx>::new();
        host.bind(&mut bus, TestId::A, mounted);
        bus.subscribe(TestId::A, EventType::Tick);
        bus.subscribe(TestId::A, EventType::Resize);

        let mut bindings = Keybindings::new();
        bindings.add(TestCtx::Nav, "next", vec!["j".into()]);
        let state = TestState {
            focused: Some(TestId::A),
            context: TestCtx::Nav,
            label: "bound".into(),
        };

        let outcome = bus.handle_event(&EventKind::Key(key("j")), &state, &bindings);
        assert_eq!(outcome.actions, vec![TestAction::Emit("next".into())]);

        let tick = bus.handle_event(&EventKind::Tick, &state, &bindings);
        assert_eq!(tick.actions, vec![TestAction::Emit("tick".into())]);

        let mut render = RenderHarness::new(10, 1);
        render.render(|frame| host.render(mounted, frame, frame.area(), &state));
        host.sync_areas(&mut bus);
        assert_eq!(
            bus.context().component_areas.get(&TestId::A).copied(),
            Some(Rect::new(0, 0, 10, 1))
        );

        host.sync_areas(&mut bus);
        assert_eq!(bus.context().component_areas.get(&TestId::A).copied(), None);
    }

    #[test]
    fn unmount_requires_unbinding_first() {
        let host = ComponentHost::<TestState, TestAction, TestId, TestCtx>::new();
        let mounted: Mounted<EchoComponent> = host.mount::<EchoComponent, _>(
            || EchoComponent {
                resets: Rc::new(Cell::new(1)),
            },
            label_props,
        );
        let mut bus = EventBus::<TestState, TestAction, TestId, TestCtx>::new();

        host.bind(&mut bus, TestId::A, mounted);
        assert_eq!(
            host.unmount(mounted),
            Err(HostLifecycleError::StillBound(TestId::A))
        );

        host.unbind(&mut bus, TestId::A);
        assert_eq!(host.unmount(mounted), Ok(()));
    }

    #[test]
    fn routed_event_prefers_command_over_raw_key() {
        let routed = RoutedEvent {
            kind: EventKind::Key(key_event(KeyCode::Char('j'))),
            command: Some("next"),
            binding_ctx: TestCtx::Nav,
            target: RouteTarget::Focused(TestId::A),
            context: &Default::default(),
        };

        match ComponentInput::from(routed) {
            ComponentInput::Command { name, ctx } => {
                assert_eq!(name, "next");
                assert_eq!(ctx, TestCtx::Nav);
            }
            other => panic!("unexpected routed input: {other:?}"),
        }
    }
}
