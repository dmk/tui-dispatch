//! Tree view component with selection and expand/collapse

use std::collections::HashSet;
use std::hash::Hash;
use std::marker::PhantomData;

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use tui_dispatch_core::{Component, EventKind};

use crate::style::{BaseStyle, ComponentStyle, Padding, ScrollbarStyle, SelectionStyle};

/// Tree node data structure
#[derive(Debug, Clone)]
pub struct TreeNode<Id, T> {
    pub id: Id,
    pub value: T,
    pub children: Vec<TreeNode<Id, T>>,
}

impl<Id, T> TreeNode<Id, T> {
    /// Create a new tree node
    pub fn new(id: Id, value: T) -> Self {
        Self {
            id,
            value,
            children: Vec::new(),
        }
    }

    /// Create a node with children
    pub fn with_children(id: Id, value: T, children: Vec<TreeNode<Id, T>>) -> Self {
        Self {
            id,
            value,
            children,
        }
    }
}

/// Branch connector rendering mode
#[derive(Debug, Clone, Copy, Default)]
pub enum TreeBranchMode {
    /// Indent with caret indicators (▸/▾)
    #[default]
    Caret,
    /// Indent with branch connectors (├─/└─) plus caret
    Branch,
}

/// Visual style for tree branches
#[derive(Debug, Clone)]
pub struct TreeBranchStyle {
    /// Connector rendering mode
    pub mode: TreeBranchMode,
    /// Indent width per depth level
    pub indent_width: usize,
    /// Style for branch connectors
    pub connector_style: Style,
    /// Style for caret glyphs
    pub caret_style: Style,
}

impl Default for TreeBranchStyle {
    fn default() -> Self {
        Self {
            mode: TreeBranchMode::default(),
            indent_width: 2,
            connector_style: Style::default(),
            caret_style: Style::default(),
        }
    }
}

/// Unified styling for TreeView
#[derive(Debug, Clone)]
pub struct TreeViewStyle {
    /// Shared base style
    pub base: BaseStyle,
    /// Selection indication styling
    pub selection: SelectionStyle,
    /// Scrollbar styling
    pub scrollbar: ScrollbarStyle,
    /// Branch rendering style
    pub branches: TreeBranchStyle,
}

impl Default for TreeViewStyle {
    fn default() -> Self {
        Self {
            base: BaseStyle {
                fg: Some(ratatui::style::Color::Reset),
                ..Default::default()
            },
            selection: SelectionStyle::default(),
            scrollbar: ScrollbarStyle::default(),
            branches: TreeBranchStyle::default(),
        }
    }
}

impl TreeViewStyle {
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

impl ComponentStyle for TreeViewStyle {
    fn base(&self) -> &BaseStyle {
        &self.base
    }
}

/// Behavior configuration for TreeView
#[derive(Debug, Clone)]
pub struct TreeViewBehavior {
    /// Show scrollbar when content exceeds viewport
    pub show_scrollbar: bool,
    /// Wrap navigation from last to first item (and vice versa)
    pub wrap_navigation: bool,
    /// Enter key toggles expand/collapse
    pub enter_toggles: bool,
    /// Space key toggles expand/collapse
    pub space_toggles: bool,
}

impl Default for TreeViewBehavior {
    fn default() -> Self {
        Self {
            show_scrollbar: true,
            wrap_navigation: false,
            enter_toggles: true,
            space_toggles: true,
        }
    }
}

/// Context for custom node rendering
pub struct TreeNodeRender<'a, Id, T> {
    pub node: &'a TreeNode<Id, T>,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
    pub is_selected: bool,
    pub available_width: usize,
    pub leading_width: usize,
    pub row_width: usize,
    pub tree_column_width: usize,
}

/// Props for TreeView component
pub struct TreeViewProps<'a, Id, T, A>
where
    Id: Clone + Eq + Hash + 'static,
{
    /// Nodes to render
    pub nodes: &'a [TreeNode<Id, T>],
    /// Currently selected node id
    pub selected_id: Option<&'a Id>,
    /// Expanded node ids
    pub expanded_ids: &'a HashSet<Id>,
    /// Whether this component has focus
    pub is_focused: bool,
    /// Unified styling
    pub style: TreeViewStyle,
    /// Behavior configuration
    pub behavior: TreeViewBehavior,
    /// Optional width override for column sizing
    #[allow(clippy::type_complexity)]
    pub measure_node: Option<&'a dyn Fn(&TreeNode<Id, T>) -> usize>,
    /// Padding to add to the widest tree column
    pub column_padding: usize,
    /// Callback to create action when selection changes
    pub on_select: fn(&Id) -> A,
    /// Callback to create action when expansion changes
    pub on_toggle: fn(&Id, bool) -> A,
    /// Render a node into a Line
    pub render_node: &'a dyn Fn(TreeNodeRender<'_, Id, T>) -> Line<'static>,
}

#[derive(Clone)]
struct FlatNode<'a, Id, T> {
    node: &'a TreeNode<Id, T>,
    depth: usize,
    parent_index: Option<usize>,
    has_children: bool,
    is_expanded: bool,
    is_last: bool,
    branch_mask: Vec<bool>,
}

/// Tree view component with selection and expand/collapse
pub struct TreeView<Id> {
    scroll_offset: usize,
    _marker: PhantomData<Id>,
}

impl<Id> Default for TreeView<Id> {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            _marker: PhantomData,
        }
    }
}

impl<Id> TreeView<Id> {
    /// Create a new TreeView
    pub fn new() -> Self {
        Self::default()
    }

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
}

impl<Id> TreeView<Id> {
    fn flatten_visible<'a, T>(
        nodes: &'a [TreeNode<Id, T>],
        expanded: &HashSet<Id>,
    ) -> Vec<FlatNode<'a, Id, T>>
    where
        Id: Clone + Eq + Hash,
    {
        fn walk<'a, Id, T>(
            nodes: &'a [TreeNode<Id, T>],
            expanded: &HashSet<Id>,
            depth: usize,
            parent_index: Option<usize>,
            branch_mask: Vec<bool>,
            out: &mut Vec<FlatNode<'a, Id, T>>,
        ) where
            Id: Clone + Eq + Hash,
        {
            for (idx, node) in nodes.iter().enumerate() {
                let is_last = idx + 1 == nodes.len();
                let has_children = !node.children.is_empty();
                let is_expanded = has_children && expanded.contains(&node.id);
                let current_index = out.len();

                out.push(FlatNode {
                    node,
                    depth,
                    parent_index,
                    has_children,
                    is_expanded,
                    is_last,
                    branch_mask: branch_mask.clone(),
                });

                if has_children && is_expanded {
                    let mut next_mask = branch_mask.clone();
                    next_mask.push(!is_last);
                    walk(
                        &node.children,
                        expanded,
                        depth + 1,
                        Some(current_index),
                        next_mask,
                        out,
                    );
                }
            }
        }

        let mut out = Vec::new();
        walk(nodes, expanded, 0, None, Vec::new(), &mut out);
        out
    }

    fn marker_prefix(marker: Option<&'static str>, is_selected: bool) -> String {
        let Some(marker) = marker else {
            return String::new();
        };
        if is_selected {
            marker.to_string()
        } else {
            " ".repeat(marker.chars().count())
        }
    }

    fn caret_prefix(
        depth: usize,
        indent_width: usize,
        has_children: bool,
        is_expanded: bool,
    ) -> (String, String) {
        let connector = " ".repeat(depth.saturating_mul(indent_width));
        let caret = if has_children {
            if is_expanded {
                "▾ "
            } else {
                "▸ "
            }
        } else {
            "  "
        };
        (connector, caret.to_string())
    }

    fn branch_prefix(
        branch_mask: &[bool],
        indent_width: usize,
        is_last: bool,
        has_children: bool,
        is_expanded: bool,
    ) -> (String, String) {
        let width = indent_width.max(2);
        let mut connector = String::new();
        for has_branch in branch_mask {
            if *has_branch {
                connector.push('│');
                connector.push_str(&" ".repeat(width.saturating_sub(1)));
            } else {
                connector.push_str(&" ".repeat(width));
            }
        }

        connector.push(if is_last { '└' } else { '├' });
        connector.push_str(&"─".repeat(width.saturating_sub(1)));

        let caret = if has_children {
            if is_expanded {
                "▾ "
            } else {
                "▸ "
            }
        } else {
            "  "
        };

        (connector, caret.to_string())
    }

    fn build_prefix<T>(style: &TreeViewStyle, node: &FlatNode<'_, Id, T>) -> (String, String) {
        match style.branches.mode {
            TreeBranchMode::Caret => Self::caret_prefix(
                node.depth,
                style.branches.indent_width,
                node.has_children,
                node.is_expanded,
            ),
            TreeBranchMode::Branch => Self::branch_prefix(
                &node.branch_mask,
                style.branches.indent_width,
                node.is_last,
                node.has_children,
                node.is_expanded,
            ),
        }
    }

    fn available_width(width: usize, prefix_len: usize, marker_len: usize) -> usize {
        width.saturating_sub(prefix_len).saturating_sub(marker_len)
    }
}

impl<Id, A> Component<A> for TreeView<Id>
where
    Id: Clone + Eq + Hash + 'static,
{
    type Props<'a> = TreeViewProps<'a, Id, String, A>;

    fn handle_event(
        &mut self,
        event: &EventKind,
        props: Self::Props<'_>,
    ) -> impl IntoIterator<Item = A> {
        if !props.is_focused {
            return None;
        }

        let visible = Self::flatten_visible(props.nodes, props.expanded_ids);
        if visible.is_empty() {
            return None;
        }

        let selected_idx = props
            .selected_id
            .and_then(|id| visible.iter().position(|n| &n.node.id == id));
        let has_selection = selected_idx.is_some();
        let current_idx = selected_idx.unwrap_or(0);
        let last_idx = visible.len().saturating_sub(1);

        let move_selection = |idx: usize| Some((props.on_select)(&visible[idx].node.id));
        let toggle_node =
            |idx: usize, expand: bool| Some((props.on_toggle)(&visible[idx].node.id, expand));

        match event {
            EventKind::Key(key) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if !has_selection {
                        return move_selection(0);
                    }
                    let next = if props.behavior.wrap_navigation && current_idx == last_idx {
                        0
                    } else {
                        (current_idx + 1).min(last_idx)
                    };
                    if next != current_idx {
                        move_selection(next)
                    } else {
                        None
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if !has_selection {
                        return move_selection(last_idx);
                    }
                    let next = if props.behavior.wrap_navigation && current_idx == 0 {
                        last_idx
                    } else {
                        current_idx.saturating_sub(1)
                    };
                    if next != current_idx {
                        move_selection(next)
                    } else {
                        None
                    }
                }
                KeyCode::Char('g') | KeyCode::Home => {
                    if current_idx != 0 || !has_selection {
                        move_selection(0)
                    } else {
                        None
                    }
                }
                KeyCode::Char('G') | KeyCode::End => {
                    if current_idx != last_idx || !has_selection {
                        move_selection(last_idx)
                    } else {
                        None
                    }
                }
                KeyCode::Left => {
                    let current = &visible[current_idx];
                    if current.has_children && current.is_expanded {
                        toggle_node(current_idx, false)
                    } else if let Some(parent_idx) = current.parent_index {
                        move_selection(parent_idx)
                    } else {
                        None
                    }
                }
                KeyCode::Right => {
                    let current = &visible[current_idx];
                    if current.has_children && !current.is_expanded {
                        toggle_node(current_idx, true)
                    } else if current.has_children && current.is_expanded {
                        let child_idx = current_idx + 1;
                        if child_idx < visible.len()
                            && visible[child_idx].parent_index == Some(current_idx)
                        {
                            move_selection(child_idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                KeyCode::Enter => {
                    let current = &visible[current_idx];
                    if props.behavior.enter_toggles && current.has_children {
                        toggle_node(current_idx, !current.is_expanded)
                    } else {
                        move_selection(current_idx)
                    }
                }
                KeyCode::Char(' ') => {
                    let current = &visible[current_idx];
                    if props.behavior.space_toggles && current.has_children {
                        toggle_node(current_idx, !current.is_expanded)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            EventKind::Scroll { delta, .. } => {
                if *delta == 0 {
                    None
                } else if *delta > 0 {
                    if !has_selection {
                        move_selection(last_idx)
                    } else if current_idx > 0 {
                        move_selection(current_idx - 1)
                    } else {
                        None
                    }
                } else if !has_selection {
                    move_selection(0)
                } else if current_idx < last_idx {
                    move_selection(current_idx + 1)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, props: Self::Props<'_>) {
        let style = &props.style;

        if let Some(bg) = style.base.bg {
            for y in area.y..area.y.saturating_add(area.height) {
                for x in area.x..area.x.saturating_add(area.width) {
                    frame.buffer_mut()[(x, y)].set_bg(bg);
                    frame.buffer_mut()[(x, y)].set_symbol(" ");
                }
            }
        }

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
        let visible = Self::flatten_visible(props.nodes, props.expanded_ids);
        let selected_idx = props
            .selected_id
            .and_then(|id| visible.iter().position(|n| &n.node.id == id));
        let selected_render_idx = selected_idx.unwrap_or(0);

        if let Some(selected_idx) = selected_idx {
            if viewport_height > 0 {
                self.ensure_visible(selected_idx, viewport_height);
            }
        }

        if viewport_height > 0 {
            let max_offset = visible.len().saturating_sub(viewport_height);
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }

        let show_scrollbar = props.behavior.show_scrollbar
            && viewport_height > 0
            && visible.len() > viewport_height
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

        let marker_len = if style.selection.disabled {
            0
        } else {
            style
                .selection
                .marker
                .map(|marker| marker.chars().count())
                .unwrap_or(0)
        };

        let row_width = list_area.width as usize;
        let max_tree_width = visible
            .iter()
            .map(|node| {
                let (connector_prefix, caret_prefix) = Self::build_prefix(style, node);
                let prefix_len = connector_prefix.chars().count() + caret_prefix.chars().count();
                let leading_width = prefix_len + marker_len;
                let available_width = Self::available_width(row_width, prefix_len, marker_len);
                let content_width = if let Some(measure_node) = props.measure_node {
                    measure_node(node.node)
                } else {
                    let line = (props.render_node)(TreeNodeRender {
                        node: node.node,
                        depth: node.depth,
                        has_children: node.has_children,
                        is_expanded: node.is_expanded,
                        is_selected: false,
                        available_width,
                        leading_width,
                        row_width,
                        tree_column_width: available_width,
                    });
                    line.width()
                };
                leading_width + content_width
            })
            .max()
            .unwrap_or(0)
            .saturating_add(props.column_padding)
            .min(row_width.saturating_sub(1).max(1));

        let items: Vec<ListItem> = visible
            .iter()
            .enumerate()
            .map(|(idx, node)| {
                let is_selected = selected_idx == Some(idx);
                let (connector_prefix, caret_prefix) = Self::build_prefix(style, node);
                let prefix_len = connector_prefix.chars().count() + caret_prefix.chars().count();
                let available_width = Self::available_width(row_width, prefix_len, marker_len);
                let leading_width = prefix_len + marker_len;
                let tree_column_width = max_tree_width
                    .saturating_sub(leading_width)
                    .min(available_width);

                let content_line = (props.render_node)(TreeNodeRender {
                    node: node.node,
                    depth: node.depth,
                    has_children: node.has_children,
                    is_expanded: node.is_expanded,
                    is_selected,
                    available_width,
                    leading_width,
                    row_width,
                    tree_column_width,
                });

                let mut spans = Vec::new();
                if !style.selection.disabled {
                    let marker_prefix = Self::marker_prefix(style.selection.marker, is_selected);
                    if !marker_prefix.is_empty() {
                        spans.push(Span::raw(marker_prefix));
                    }
                }
                if !connector_prefix.is_empty() {
                    spans.push(Span::styled(
                        connector_prefix,
                        style.branches.connector_style,
                    ));
                }
                if !caret_prefix.is_empty() {
                    spans.push(Span::styled(caret_prefix, style.branches.caret_style));
                }
                spans.extend(content_line.spans.iter().cloned());
                let display_line = Line::from(spans);

                if style.selection.disabled {
                    ListItem::new(display_line)
                } else {
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

        let highlight_style = if style.selection.disabled {
            Style::default()
        } else {
            style.selection.style.unwrap_or_default()
        };
        let list = List::new(items).highlight_style(highlight_style);

        let selected = if visible.is_empty() || selected_idx.is_none() {
            None
        } else {
            Some(selected_render_idx)
        };
        let mut state = ListState::default().with_selected(selected);
        *state.offset_mut() = self.scroll_offset;

        frame.render_stateful_widget(list, list_area, &mut state);

        if let Some(scrollbar_area) = scrollbar_area {
            let scrollbar = style.scrollbar.build(ScrollbarOrientation::VerticalRight);
            let scrollbar_len = visible
                .len()
                .saturating_sub(viewport_height)
                .saturating_add(1);
            let mut scrollbar_state = ScrollbarState::new(scrollbar_len)
                .position(self.scroll_offset)
                .viewport_content_length(viewport_height.max(1));
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tui_dispatch_core::testing::key;

    #[derive(Debug, Clone, PartialEq)]
    enum TestAction {
        Select(String),
        Toggle(String, bool),
    }

    fn select_action(id: &str) -> TestAction {
        TestAction::Select(id.to_owned())
    }

    fn toggle_action(id: &str, expanded: bool) -> TestAction {
        TestAction::Toggle(id.to_owned(), expanded)
    }

    fn render_node(ctx: TreeNodeRender<'_, String, String>) -> Line<'static> {
        Line::raw(ctx.node.value.clone())
    }

    fn sample_tree() -> Vec<TreeNode<String, String>> {
        vec![TreeNode::with_children(
            "root".to_string(),
            "Root".to_string(),
            vec![TreeNode::new("child".to_string(), "Child".to_string())],
        )]
    }

    fn props<'a>(
        nodes: &'a [TreeNode<String, String>],
        selected: Option<&'a String>,
        expanded: &'a HashSet<String>,
    ) -> TreeViewProps<'a, String, String, TestAction> {
        TreeViewProps {
            nodes,
            selected_id: selected,
            expanded_ids: expanded,
            is_focused: true,
            style: TreeViewStyle::borderless(),
            behavior: TreeViewBehavior::default(),
            measure_node: None,
            column_padding: 0,
            on_select: |id| select_action(id),
            on_toggle: |id, expanded| toggle_action(id, expanded),
            render_node: &render_node,
        }
    }

    #[test]
    fn test_expand_on_right() {
        let mut view: TreeView<String> = TreeView::new();
        let nodes = sample_tree();
        let expanded = HashSet::new();

        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Key(key("right")),
                props(&nodes, None, &expanded),
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Toggle("root".into(), true)]);
    }

    #[test]
    fn test_collapse_on_left() {
        let mut view: TreeView<String> = TreeView::new();
        let nodes = sample_tree();
        let mut expanded = HashSet::new();
        expanded.insert("root".to_string());
        let selected = Some(&nodes[0].id);

        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Key(key("left")),
                props(&nodes, selected, &expanded),
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Toggle("root".into(), false)]);
    }

    #[test]
    fn test_select_child_with_down() {
        let mut view: TreeView<String> = TreeView::new();
        let nodes = sample_tree();
        let mut expanded = HashSet::new();
        expanded.insert("root".to_string());
        let selected = Some(&nodes[0].id);

        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Key(key("down")),
                props(&nodes, selected, &expanded),
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select("child".into())]);
    }

    #[test]
    fn test_select_parent_with_left() {
        let mut view: TreeView<String> = TreeView::new();
        let nodes = sample_tree();
        let mut expanded = HashSet::new();
        expanded.insert("root".to_string());
        let selected = Some(&nodes[0].children[0].id);

        let actions: Vec<_> = view
            .handle_event(
                &EventKind::Key(key("left")),
                props(&nodes, selected, &expanded),
            )
            .into_iter()
            .collect();

        assert_eq!(actions, vec![TestAction::Select("root".into())]);
    }
}
