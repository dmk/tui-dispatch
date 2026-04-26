---
title: Pre-built Widgets
description: Reusable widgets from `tui-dispatch-components`
---

The `tui-dispatch-components` crate provides reusable widgets built on top of `ratatui`.

These widgets are optional. You can use them directly, compose them into your own widgets, or ignore them entirely and render your own UI.

If you have not read it yet, start with [Components Overview](/tui-dispatch/components/).

## Installation

```toml
[dependencies]
tui-dispatch-components = "0.6.0"
```

## Usage Modes

Most widgets can be used in more than one way:

| Widget | Render-only helper | `Component<A>` | `InteractiveComponent<A, Ctx>` |
| --- | --- | --- | --- |
| `SelectList` | Yes | Yes | Yes |
| `ScrollView` | Yes | Yes | Yes |
| `TextInput` | Yes | Yes | Yes |
| `TreeView` | Yes | Yes | Yes |
| `StatusBar` | No | Yes | No |
| `Modal` | No | Yes | No |

Recommended starting point:

- use `render_widget(...)` plus `*RenderProps` when another layer already owns state and input
- use `Component<A>` when raw `EventKind` handling is enough
- use `InteractiveComponent<A, Ctx>` when you want routed input and `HandlerResponse`

See [View Components](/tui-dispatch/components/custom/) and [Interactive Widgets](/tui-dispatch/components/interactive/) for the architectural side.

## Shared Styling Model

All widget styles embed `BaseStyle` via `ComponentStyle`, so borders, padding, and base colors stay consistent across widgets.

Useful shared types:

- `BaseStyle`
- `BorderStyle`
- `Padding`
- `ScrollbarStyle`
- `SelectionStyle`

## SelectList

`SelectList` renders a vertical list and manages viewport-local scroll state.

Render-only usage:

```rust
use ratatui::text::Line;
use tui_dispatch_components::{
    SelectList, SelectListBehavior, SelectListRenderProps, SelectListStyle,
};

let items = vec![
    Line::raw("Option 1"),
    Line::raw("Option 2"),
    Line::raw("Option 3"),
];
let render_item = |line: &Line<'static>| line.clone();

let mut list = SelectList::new();
list.render_widget(
    frame,
    area,
    SelectListRenderProps {
        items: &items,
        count: items.len(),
        selected: state.selected,
        is_focused: state.focused,
        style: SelectListStyle::default(),
        behavior: SelectListBehavior::default(),
        render_item: &render_item,
    },
);
```

When you want it to emit actions, switch to `SelectListProps` and call either:

- `Component::handle_event(...)` for raw key handling
- `InteractiveComponent::update(...)` for routed commands and `HandlerResponse`

Useful behavior flags:

- `show_scrollbar`
- `wrap_navigation`

## ScrollView

`ScrollView` manages a vertical viewport and asks you to render only the visible content.

Render-only usage:

```rust
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use tui_dispatch_components::{
    ScrollView, ScrollViewBehavior, ScrollViewRenderProps, ScrollViewStyle, VisibleRange,
};

let mut render_content = |frame: &mut Frame, area: Rect, range: VisibleRange| {
    let end = range.end.min(lines.len());
    let start = range.start.min(end);
    frame.render_widget(Paragraph::new(lines[start..end].to_vec()), area);
};

let mut scroll = ScrollView::new();
scroll.render_widget(
    frame,
    area,
    ScrollViewRenderProps {
        content_height: lines.len(),
        scroll_offset: state.scroll_offset,
        is_focused: state.focused,
        style: ScrollViewStyle::default(),
        behavior: ScrollViewBehavior::default(),
        render_content: &mut render_content,
    },
);
```

When you want scrolling actions, use `ScrollViewProps` with `on_scroll`.

Useful behavior flags:

- `show_scrollbar`
- `scroll_step`
- `page_step`

## TextInput

`TextInput` keeps cursor position locally and emits value changes through callbacks.

Render-only usage:

```rust
use tui_dispatch_components::{TextInput, TextInputRenderProps, TextInputStyle};

let mut input = TextInput::new();
input.render_widget(
    frame,
    area,
    TextInputRenderProps {
        value: &state.query,
        placeholder: "Type to search",
        is_focused: state.focused,
        style: TextInputStyle::default(),
    },
);
```

Interactive usage uses `TextInputProps`:

```rust
use std::rc::Rc;
use tui_dispatch_components::{InteractiveComponent, ComponentInput, TextInput, TextInputProps};

let response = <TextInput as InteractiveComponent<Action>>::update(
    &mut input,
    ComponentInput::Key(key_event),
    TextInputProps {
        value: &state.query,
        placeholder: "Type to search",
        is_focused: true,
        style: TextInputStyle::default(),
        on_change: Rc::new(Action::QueryChanged),
        on_submit: Rc::new(|_| Action::Submit),
        on_cursor_move: None,
        on_cancel: None,
    },
);
```

`TextInput` recognises a fixed set of [routed commands](/components/host/) when
focused: `move_backward`, `move_forward`, `move_word_backward`,
`move_word_forward`, `move_home`, `move_end`, `delete_backward`,
`delete_forward`, `delete_word_backward`, `delete_word_forward`, `submit`, and
`cancel`. Bind these to keys via `BindingContext` and the host adapter
delivers them as `ComponentInput::Command` so the same widget works with
arbitrary keymaps. `cancel` only emits an action if `on_cancel` is set;
otherwise it is left for the bus / app to handle.

`TextInput` already uses `needs_render` for cursor-only changes in its interactive path.

## TreeView

`TreeView` renders hierarchical data with selection and expand/collapse state supplied through props.

Render-only usage:

```rust
use std::collections::HashSet;
use ratatui::text::Line;
use tui_dispatch_components::{
    TreeNode, TreeView, TreeViewBehavior, TreeViewRenderProps, TreeViewStyle,
};

let nodes = vec![TreeNode::with_children(
    "root".to_string(),
    "Root".to_string(),
    vec![TreeNode::new("child".to_string(), "Child".to_string())],
)];
let expanded = HashSet::from(["root".to_string()]);
let render_node = |ctx| Line::raw(ctx.node.value.as_str());

let mut tree: TreeView<String> = TreeView::new();
tree.render_widget(
    frame,
    area,
    TreeViewRenderProps {
        nodes: &nodes,
        selected_id: state.selected.as_ref(),
        expanded_ids: &expanded,
        is_focused: state.focused,
        style: TreeViewStyle::default(),
        behavior: TreeViewBehavior::default(),
        measure_node: None,
        column_padding: 0,
        render_node: &render_node,
    },
);
```

When you want selection and toggle actions, use `TreeViewProps` with `on_select` and `on_toggle`.

Useful behavior flags:

- `show_scrollbar`
- `wrap_navigation`
- `enter_toggles`
- `space_toggles`

## StatusBar

`StatusBar` is a plain `Component<A>` that renders left, center, and right sections.

```rust
use tui_dispatch::Component;
use tui_dispatch_components::{
    StatusBar, StatusBarHint, StatusBarItem, StatusBarProps, StatusBarSection, StatusBarStyle,
};

let left = [StatusBarItem::text("NORMAL")];
let center = [StatusBarItem::text("No file")];
let hints = [
    StatusBarHint::new("/", "search"),
    StatusBarHint::new("q", "quit"),
];

let mut status = StatusBar::new();
<StatusBar as Component<()>>::render(
    &mut status,
    frame,
    area,
    StatusBarProps {
        left: StatusBarSection::items(&left),
        center: StatusBarSection::items(&center),
        right: StatusBarSection::hints(&hints),
        style: StatusBarStyle::default(),
        is_focused: false,
    },
);
```

## Modal

`Modal` dims the background and renders a child view into a modal area. It currently uses the plain `Component<A>` contract.

```rust
use std::rc::Rc;
use ratatui::{layout::Rect, Frame};
use tui_dispatch::Component;
use tui_dispatch_components::{
    centered_rect, Modal, ModalBehavior, ModalProps, ModalStyle,
};

let modal_area = centered_rect(60, 12, frame.area());
let mut render_content = |frame: &mut Frame, content_area: Rect| {
    render_dialog(frame, content_area, state);
};

let mut modal = Modal::new();
<Modal as Component<Action>>::render(
    &mut modal,
    frame,
    frame.area(),
    ModalProps {
        is_open: state.show_modal,
        is_focused: true,
        area: modal_area,
        style: ModalStyle::default(),
        behavior: ModalBehavior::default(),
        on_close: Rc::new(|| Action::CloseModal),
        render_content: &mut render_content,
    },
);
```

Use `Component::handle_event(...)` if you want `Esc` or backdrop clicks to emit a close action.

## Choosing Between `*RenderProps` and `*Props`

Use render-only props when:

- the parent already owns all interaction and state updates
- you only need the widget for layout and rendering

Use action-emitting props when:

- the widget should emit app actions directly
- you want the widget to own local UI mechanics like cursor or scroll state

## See Also

- [Components Overview](/tui-dispatch/components/)
- [View Components](/tui-dispatch/components/custom/)
- [Interactive Widgets](/tui-dispatch/components/interactive/)
- [Component Host](/tui-dispatch/components/host/)
