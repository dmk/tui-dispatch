//! Scrollable selection list component

use std::marker::PhantomData;
use std::rc::Rc;

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use tui_dispatch_core::{Component, EventKind, HandlerResponse};

use crate::style::{BaseStyle, ComponentStyle, Padding, ScrollbarStyle, SelectionStyle};
use crate::{ComponentDebugEntry, ComponentDebugState, ComponentInput, InteractiveComponent};

/// Unified styling for SelectList
#[derive(Debug, Clone)]
pub struct SelectListStyle {
    /// Shared base style
    pub base: BaseStyle,
    /// Selection indication styling
    pub selection: SelectionStyle,
    /// Scrollbar styling
    pub scrollbar: ScrollbarStyle,
}

impl Default for SelectListStyle {
    fn default() -> Self {
        Self {
            base: BaseStyle {
                fg: Some(ratatui::style::Color::Reset),
                ..Default::default()
            },
            selection: SelectionStyle::default(),
            scrollbar: ScrollbarStyle::default(),
        }
    }
}

impl SelectListStyle {
    /// Create a style with no border
    pub fn borderless() -> Self {
        let mut style = Self::default();
        style.base.border = None;
        style
    }

    /// Create a minimal style (no border, no padding)
    pub fn minimal() -> Self {
        let mut style = Self::default();
        style.base.border = None;
        style.base.padding = Padding::default();
        style
    }
}

impl ComponentStyle for SelectListStyle {
    fn base(&self) -> &BaseStyle {
        &self.base
    }
}

/// Behavior configuration for SelectList
#[derive(Debug, Clone)]
pub struct SelectListBehavior {
    /// Show scrollbar when content exceeds viewport
    pub show_scrollbar: bool,
    /// Wrap navigation from last to first item (and vice versa)
    pub wrap_navigation: bool,
}

impl Default for SelectListBehavior {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            wrap_navigation: false,
        }
    }
}

/// Callback to create an action when the selected index changes.
pub type SelectListCallback<A> = Rc<dyn Fn(usize) -> A>;

/// Props for SelectList component
#[derive(Clone)]
pub struct SelectListProps<'a, T, A> {
    /// Items to render
    pub items: &'a [T],
    /// Total count (may differ from items.len() for virtual lists)
    pub count: usize,
    /// Currently selected index
    pub selected: usize,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: SelectListStyle,
    /// Behavior configuration
    pub behavior: SelectListBehavior,
    /// Callback to create action when selection changes
    pub on_select: SelectListCallback<A>,
    /// Render a single item into a Line
    pub render_item: &'a dyn Fn(&T) -> Line<'static>,
}

/// Render-only props for SelectList
pub struct SelectListRenderProps<'a, T> {
    /// Items to render
    pub items: &'a [T],
    /// Total count (may differ from items.len() for virtual lists)
    pub count: usize,
    /// Currently selected index
    pub selected: usize,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: SelectListStyle,
    /// Behavior configuration
    pub behavior: SelectListBehavior,
    /// Render a single item into a Line
    pub render_item: &'a dyn Fn(&T) -> Line<'static>,
}

impl<'a, T, A> SelectListProps<'a, T, A> {
    /// Create props with sensible defaults
    ///
    /// Sets `count` to `items.len()`, `is_focused` to `true`, and uses default style/behavior.
    pub fn new(
        items: &'a [T],
        selected: usize,
        on_select: SelectListCallback<A>,
        render_item: &'a dyn Fn(&T) -> Line<'static>,
    ) -> Self {
        Self {
            items,
            count: items.len(),
            selected,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select,
            render_item,
        }
    }
}

/// A scrollable selection list with keyboard navigation
///
/// Handles j/k/up/down for navigation and enter for selection.
/// Generic over item type `T` - provide a `render_item` callback to convert to Lines.
pub struct SelectList<Item = Line<'static>> {
    /// Scroll offset for viewport
    scroll_offset: usize,
    _marker: PhantomData<fn() -> Item>,
}

impl<Item> Default for SelectList<Item> {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            _marker: PhantomData,
        }
    }
}

impl<Item> SelectList<Item> {
    /// Create a new SelectList
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the widget without requiring selection callbacks.
    pub fn render_widget(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        props: SelectListRenderProps<'_, Item>,
    ) {
        self.render_with(frame, area, props);
    }

    /// Ensure the selected index is visible within the viewport
    fn ensure_visible(&mut self, selected: usize, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }

        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        } else if selected >= self.scroll_offset + viewport_height {
            self.scroll_offset = selected.saturating_sub(viewport_height - 1);
        }
    }

    fn next_index(&self, selected: usize, len: usize, wrap_navigation: bool) -> usize {
        if wrap_navigation && selected == len.saturating_sub(1) {
            0
        } else {
            (selected + 1).min(len.saturating_sub(1))
        }
    }

    fn prev_index(&self, selected: usize, len: usize, wrap_navigation: bool) -> usize {
        if wrap_navigation && selected == 0 {
            len.saturating_sub(1)
        } else {
            selected.saturating_sub(1)
        }
    }

    fn select_action<A>(
        &self,
        selected: usize,
        next: usize,
        on_select: &dyn Fn(usize) -> A,
    ) -> Option<A> {
        (next != selected).then(|| on_select(next))
    }

    fn handle_navigation<A>(
        &mut self,
        command: NavigationCommand,
        props: &SelectListProps<'_, Item, A>,
    ) -> Option<A> {
        if !props.is_focused || props.count == 0 {
            return None;
        }

        let len = props.count;

        match command {
            NavigationCommand::Next => self.select_action(
                props.selected,
                self.next_index(props.selected, len, props.behavior.wrap_navigation),
                props.on_select.as_ref(),
            ),
            NavigationCommand::Prev => self.select_action(
                props.selected,
                self.prev_index(props.selected, len, props.behavior.wrap_navigation),
                props.on_select.as_ref(),
            ),
            NavigationCommand::First => {
                self.select_action(props.selected, 0, props.on_select.as_ref())
            }
            NavigationCommand::Last => self.select_action(
                props.selected,
                len.saturating_sub(1),
                props.on_select.as_ref(),
            ),
            NavigationCommand::Select => Some((props.on_select.as_ref())(props.selected)),
        }
    }

    fn render_with(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        props: SelectListRenderProps<'_, Item>,
    ) {
        let style = &props.style;

        // Fill background if specified
        if let Some(bg) = style.base.bg {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    frame.buffer_mut()[(x, y)].set_bg(bg);
                    frame.buffer_mut()[(x, y)].set_symbol(" ");
                }
            }
        }

        // Apply padding
        let content_area = Rect {
            x: area.x + style.base.padding.left,
            y: area.y + style.base.padding.top,
            width: area.width.saturating_sub(style.base.padding.horizontal()),
            height: area.height.saturating_sub(style.base.padding.vertical()),
        };

        let mut inner_area = content_area;
        if let Some(border) = &style.base.border {
            let block = Block::default()
                .borders(border.borders)
                .border_style(border.style_for_focus(props.is_focused));
            inner_area = block.inner(content_area);
            frame.render_widget(block, content_area);
        }

        let viewport_height = inner_area.height as usize;
        let render_selected = props.selected.min(props.items.len().saturating_sub(1));

        // Ensure selected item is visible
        if !props.items.is_empty() && viewport_height > 0 {
            self.ensure_visible(render_selected, viewport_height);
        }

        if viewport_height > 0 {
            let max_offset = props.count.saturating_sub(viewport_height);
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }

        let show_scrollbar = props.behavior.show_scrollbar
            && viewport_height > 0
            && props.count > viewport_height
            && inner_area.width > 1;
        let mut list_area = inner_area;
        let scrollbar_area = if show_scrollbar {
            let scrollbar_area = Rect {
                x: inner_area.x + inner_area.width.saturating_sub(1),
                width: 1,
                ..inner_area
            };
            list_area.width = list_area.width.saturating_sub(1);
            Some(scrollbar_area)
        } else {
            None
        };

        // Build list items with optional selection styling
        let items: Vec<ListItem> = props
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == render_selected;
                let line = (props.render_item)(item);

                // Apply selection styling unless disabled
                if style.selection.disabled {
                    ListItem::new(line)
                } else {
                    // Build the line with optional marker
                    let display_line = if let Some(marker) = style.selection.marker {
                        let prefix = if is_selected {
                            marker
                        } else {
                            &"  "[..marker.len().min(2)]
                        };
                        let mut spans = vec![Span::raw(prefix)];
                        spans.extend(line.spans.iter().cloned());
                        Line::from(spans)
                    } else {
                        line
                    };

                    // Apply selection style (or base fg color for non-selected)
                    let item_style = if is_selected {
                        style.selection.style.unwrap_or_default()
                    } else {
                        let mut s = Style::default();
                        if let Some(fg) = style.base.fg {
                            s = s.fg(fg);
                        }
                        s
                    };

                    ListItem::new(display_line).style(item_style)
                }
            })
            .collect();

        // Create the list widget
        let highlight_style = if style.selection.disabled {
            Style::default()
        } else {
            style.selection.style.unwrap_or_default()
        };
        let list = List::new(items).highlight_style(highlight_style);

        // Use ListState to handle scroll offset
        let selected = if props.items.is_empty() {
            None
        } else {
            Some(render_selected)
        };
        let mut state = ListState::default().with_selected(selected);
        *state.offset_mut() = self.scroll_offset;

        frame.render_stateful_widget(list, list_area, &mut state);

        if let Some(scrollbar_area) = scrollbar_area {
            let scrollbar = style.scrollbar.build(ScrollbarOrientation::VerticalRight);
            let scrollbar_len = props
                .count
                .saturating_sub(viewport_height)
                .saturating_add(1);
            let mut scrollbar_state = ScrollbarState::new(scrollbar_len)
                .position(self.scroll_offset)
                .viewport_content_length(viewport_height.max(1));
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}

#[derive(Clone, Copy)]
enum NavigationCommand {
    Next,
    Prev,
    First,
    Last,
    Select,
}

impl<Item, A> Component<A> for SelectList<Item> {
    type Props<'a>
        = SelectListProps<'a, Item, A>
    where
        Item: 'a;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        if !props.is_focused {
            return None;
        }

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.handle_navigation(NavigationCommand::Next, &props)
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.handle_navigation(NavigationCommand::Prev, &props)
                }
                KeyCode::Char('g') | KeyCode::Home => {
                    self.handle_navigation(NavigationCommand::First, &props)
                }
                KeyCode::Char('G') | KeyCode::End => {
                    self.handle_navigation(NavigationCommand::Last, &props)
                }
                KeyCode::Enter => self.handle_navigation(NavigationCommand::Select, &props),
                _ => None,
            },
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        self.render_with(
            frame,
            area,
            SelectListRenderProps {
                items: props.items,
                count: props.count,
                selected: props.selected,
                is_focused: props.is_focused,
                style: props.style,
                behavior: props.behavior,
                render_item: props.render_item,
            },
        );
    }
}

impl<Item> ComponentDebugState for SelectList<Item> {
    fn debug_state(&self) -> Vec<ComponentDebugEntry> {
        vec![ComponentDebugEntry::new(
            "scroll_offset",
            self.scroll_offset.to_string(),
        )]
    }
}

impl<Item, A, Ctx> InteractiveComponent<A, Ctx> for SelectList<Item> {
    type Props<'a>
        = SelectListProps<'a, Item, A>
    where
        Item: 'a;

    fn update(
        &mut self,
        input: ComponentInput<'_, Ctx>,
        props: Self::Props<'_>,
    ) -> HandlerResponse<A> {
        if !props.is_focused {
            return HandlerResponse::ignored();
        }

        let action = match input {
            ComponentInput::Command { name, .. } => match name {
                "next" | "down" => self.handle_navigation(NavigationCommand::Next, &props),
                "prev" | "up" => self.handle_navigation(NavigationCommand::Prev, &props),
                "first" | "home" => self.handle_navigation(NavigationCommand::First, &props),
                "last" | "end" => self.handle_navigation(NavigationCommand::Last, &props),
                "select" | "confirm" => self.handle_navigation(NavigationCommand::Select, &props),
                _ => None,
            },
            ComponentInput::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.handle_navigation(NavigationCommand::Next, &props)
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.handle_navigation(NavigationCommand::Prev, &props)
                }
                KeyCode::Char('g') | KeyCode::Home => {
                    self.handle_navigation(NavigationCommand::First, &props)
                }
                KeyCode::Char('G') | KeyCode::End => {
                    self.handle_navigation(NavigationCommand::Last, &props)
                }
                KeyCode::Enter => self.handle_navigation(NavigationCommand::Select, &props),
                _ => None,
            },
            _ => None,
        };

        match action {
            Some(action) => HandlerResponse::action(action),
            None => HandlerResponse::ignored(),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        <Self as Component<A>>::render(self, frame, area, props);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui_dispatch_core::testing::{key, RenderHarness};

    #[derive(Debug, Clone, PartialEq)]
    enum TestAction {
        Select(usize),
    }

    fn make_items() -> Vec<Line<'static>> {
        vec![
            Line::raw("Item 0"),
            Line::raw("Item 1"),
            Line::raw("Item 2"),
        ]
    }

    fn render_item(item: &Line<'static>) -> Line<'static> {
        item.clone()
    }

    #[test]
    fn test_navigate_down() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select(1)]);
    }

    #[test]
    fn test_navigate_up() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 2,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("k")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select(1)]);
    }

    #[test]
    fn test_navigate_at_bounds() {
        let mut list = SelectList::new();
        let items = make_items();

        // At top, going up should not emit
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("k")), props)
            .into_iter()
            .collect();
        assert!(actions.is_empty());

        // At bottom, going down should not emit
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 2,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_wrap_navigation() {
        let mut list = SelectList::new();
        let items = make_items();

        // At top with wrap, going up should go to bottom
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior {
                wrap_navigation: true,
                ..Default::default()
            },
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("k")), props)
            .into_iter()
            .collect();
        assert_eq!(actions, vec![TestAction::Select(2)]);

        // At bottom with wrap, going down should go to top
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 2,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior {
                wrap_navigation: true,
                ..Default::default()
            },
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };
        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();
        assert_eq!(actions, vec![TestAction::Select(0)]);
    }

    #[test]
    fn test_unfocused_ignores_events() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: false,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("j")), props)
            .into_iter()
            .collect();

        assert!(actions.is_empty());
    }

    #[test]
    fn test_unfocused_ignores_commands() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 0,
            is_focused: false,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };

        let response = <SelectList as InteractiveComponent<TestAction, ()>>::update(
            &mut list,
            ComponentInput::Command {
                name: "next",
                ctx: (),
            },
            props,
        );

        assert!(response.actions.is_empty());
        assert!(!response.consumed);
        assert!(!response.needs_render);
    }

    #[test]
    fn test_enter_selects_current() {
        let mut list = SelectList::new();
        let items = make_items();
        let props = SelectListProps {
            items: &items,
            count: items.len(),
            selected: 1,
            is_focused: true,
            style: SelectListStyle::default(),
            behavior: SelectListBehavior::default(),
            on_select: Rc::new(TestAction::Select),
            render_item: &render_item,
        };

        let actions: Vec<_> = list
            .handle_event(&EventKind::Key(key("enter")), props)
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select(1)]);
    }

    #[test]
    fn test_render() {
        let mut render = RenderHarness::new(30, 10);
        let mut list = SelectList::new();
        let items = make_items();

        let output = render.render_to_string_plain(|frame| {
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: 1,
                is_focused: true,
                style: SelectListStyle::default(),
                behavior: SelectListBehavior::default(),
                on_select: Rc::new(|_| ()),
                render_item: &render_item,
            };
            <SelectList as Component<()>>::render(&mut list, frame, frame.area(), props);
        });

        assert!(output.contains("Item 0"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
    }

    #[test]
    fn test_render_without_selection_styling() {
        let mut render = RenderHarness::new(30, 10);
        let mut list = SelectList::new();
        let items = make_items();

        let output = render.render_to_string_plain(|frame| {
            let props = SelectListProps {
                items: &items,
                count: items.len(),
                selected: 1,
                is_focused: true,
                style: SelectListStyle {
                    selection: SelectionStyle::disabled(),
                    ..Default::default()
                },
                behavior: SelectListBehavior::default(),
                on_select: Rc::new(|_| ()),
                render_item: &render_item,
            };
            <SelectList as Component<()>>::render(&mut list, frame, frame.area(), props);
        });

        // Should render items without markers
        assert!(output.contains("Item 0"));
        assert!(output.contains("Item 1"));
        assert!(output.contains("Item 2"));
        // Should not contain selection marker
        assert!(!output.contains(">"));
    }
}
