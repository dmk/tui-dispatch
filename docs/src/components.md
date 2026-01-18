# Pre-built Components

The `tui-dispatch-components` crate provides reusable UI components that integrate with tui-dispatch patterns. All components implement the `Component<A>` trait.

## Installation

```toml
[dependencies]
tui-dispatch-components = "0.4"
```

## Available Components

- **SelectList** - Scrollable selection list with keyboard navigation
- **ScrollView** - Scrollable view for pre-wrapped lines
- **StatusBar** - Left/center/right status bar with hints
- **TextInput** - Single-line text input with cursor
- **TreeView** - Hierarchical tree view with selection and expand/collapse
- **Modal** - Overlay with dimmed background

## Design Philosophy

Components are designed around three principles:

1. **Unified styling** - Each component style embeds `BaseStyle` via `ComponentStyle`
2. **Headless rendering** - Components handle layout/interaction; you render inner rows
3. **Behavior configuration** - Runtime behavior (scrollbar, wrap-around) is configurable

### ComponentStyle Trait

All component styles implement `ComponentStyle`, ensuring consistent access to common fields:

```rust
pub trait ComponentStyle {
    fn base(&self) -> &BaseStyle;
}
```

This enables generic code to work with any component style.

## SelectList

A scrollable list with keyboard navigation (j/k, arrows, g/G, Enter).

### Basic Usage

```rust
use tui_dispatch_components::{
    Line, SelectList, SelectListBehavior, SelectListProps, SelectListStyle,
};

let items: Vec<Line> = vec![
    Line::raw("Option 1"),
    Line::raw("Option 2"),
    Line::raw("Option 3"),
];
let render_item = |item: &Line| item.clone();

// Using the helper constructor (sets count, is_focused, style, behavior to defaults)
let props = SelectListProps::new(
    &items,
    state.selected,
    |idx| Action::Select(idx),
    &render_item,
);

list.render(frame, area, props);
```

Or with full control:

```rust
let props = SelectListProps {
    items: &items,
    count: items.len(),
    selected: state.selected,
    is_focused: true,
    style: SelectListStyle::default(),
    behavior: SelectListBehavior::default(),
    on_select: |idx| Action::Select(idx),
    render_item: &render_item,
};
```

### Custom Item Rendering

Since you provide `Line` items, you have full control over styling. Use the built-in
`highlight_substring` utility for search highlighting:

```rust
use tui_dispatch_components::{highlight_substring, Style, Color, Modifier};

fn render_items(items: &[MyItem], query: &str) -> Vec<Line<'static>> {
    let base = Style::default();
    let highlight = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

    items.iter().map(|item| {
        highlight_substring(&item.name, query, base, highlight)
    }).collect()
}
```

### Styling

```rust
use tui_dispatch_components::{
    BaseStyle, BorderStyle, Padding, ScrollbarStyle, SelectionStyle, SelectListStyle,
};
use ratatui::style::{Color, Style};

let style = SelectListStyle {
    base: BaseStyle {
        border: Some(BorderStyle::default()),
        padding: Padding::xy(1, 0),
        bg: Some(Color::Rgb(30, 30, 40)),
        fg: Some(Color::Reset),
    },
    selection: SelectionStyle {
        style: Some(Style::default().fg(Color::Cyan)),
        marker: Some("> "),
        disabled: false,
    },
    scrollbar: ScrollbarStyle::default(),
};
```

#### Disabling Selection Styling

When you want full control over how selected items look:

```rust
let style = SelectListStyle {
    selection: SelectionStyle::disabled(),
    ..Default::default()
};

// Now you handle selection indication in your Line rendering
let items: Vec<Line> = data.iter().enumerate().map(|(i, item)| {
    if i == selected {
        Line::styled(&item.name, Style::default().fg(Color::Yellow))
    } else {
        Line::raw(&item.name)
    }
}).collect();
```

### Behavior Configuration

```rust
let behavior = SelectListBehavior {
    show_scrollbar: true,      // Show scrollbar when content overflows
    wrap_navigation: true,     // Wrap from last to first item
};
```

### Future: Virtual Lists

The `count` field enables future virtualization support:

```rust
// When you have sparse/virtual data:
let props = SelectListProps {
    items: &loaded_items,      // Only loaded items
    count: total_count,        // Total items (may differ from items.len())
    // ...
};
```

## ScrollView

A scrollable view for pre-wrapped lines with a controlled scroll offset.

### Basic Usage

```rust
use tui_dispatch_components::{
    Line, ScrollView, ScrollViewBehavior, ScrollViewProps, ScrollViewStyle,
};

let lines: Vec<Line> = state.lines.iter().map(|line| Line::raw(line)).collect();

let props = ScrollViewProps {
    lines: &lines,
    content_len: state.total_lines,
    line_offset: state.slice_start,
    scroll_offset: state.scroll_offset,
    is_focused: state.focus == Focus::Content,
    style: ScrollViewStyle::default(),
    behavior: ScrollViewBehavior::default(),
    on_scroll: Action::ScrollTo,
};

scroll_view.render(frame, area, props);
```

The `line_offset` and `content_len` fields let you virtualize large buffers by
providing only the visible slice of lines.

## StatusBar

A left/center/right status bar for mode info, ephemeral messages, and key hints.

### Basic Usage

```rust
use tui_dispatch_components::{
    StatusBar, StatusBarHint, StatusBarItem, StatusBarProps, StatusBarSection, StatusBarStyle,
};

let left = [StatusBarItem::text("NORMAL")];
let center = [StatusBarItem::text("No file")];
let hints = [
    StatusBarHint::new("/", "search"),
    StatusBarHint::new("F1", "help"),
];

let props = StatusBarProps {
    left: StatusBarSection::items(&left),
    center: StatusBarSection::items(&center),
    right: StatusBarSection::hints(&hints),
    style: StatusBarStyle::default(),
    is_focused: false,
};

status_bar.render(frame, area, props);
```

Sections are clipped to the available width, with the center section placed
between the left and right content.

## TextInput

A single-line text input with cursor movement and editing.

### Basic Usage

```rust
use tui_dispatch_components::{TextInput, TextInputProps, TextInputStyle};

let props = TextInputProps {
    value: &state.input_value,
    placeholder: "Type here...",
    is_focused: true,
    style: TextInputStyle::default(),
    on_change: |s| Action::InputChange(s),
    on_submit: |s| Action::InputSubmit(s),
    on_cursor_move: Some(|_| Action::Render),
};

input.render(frame, area, props);
```

Use `on_cursor_move` to trigger a re-render for cursor-only movement (left/right/home/end, word movement) when the text value doesn't change.

### Styling

```rust
use tui_dispatch_components::{BaseStyle, BorderStyle, Padding, TextInputStyle};
use ratatui::style::{Color, Style};

let style = TextInputStyle {
    base: BaseStyle {
        border: Some(BorderStyle::default()),
        padding: Padding::xy(1, 0),
        bg: Some(Color::Rgb(50, 50, 60)),
        fg: Some(Color::White),
    },
    placeholder_style: Some(Style::default().fg(Color::DarkGray)),
    cursor_style: None,
};
```

### Keyboard Handling

Full readline/emacs-style keybindings:

**Basic:**
- **Characters**: Insert at cursor
- **Backspace/Delete**: Remove characters
- **Left/Right**: Move by character
- **Home/End**: Move to start/end
- **Enter**: Submit

**Emacs Movement:**
- **Ctrl+A/E**: Move to start/end
- **Ctrl+B/F**: Move backward/forward by character
- **Ctrl+Left/Right**: Move by word (Mac-friendly)
- **Alt+B/F**: Move by word (when terminal configured)

**Deletion:**
- **Ctrl+U**: Clear entire line
- **Ctrl+K**: Kill to end of line
- **Ctrl+W**: Delete word backward
- **Ctrl+D**: Delete character at cursor
- **Ctrl+H**: Backspace (alternative)
- **Alt+D**: Delete word forward
- **Alt+Backspace**: Delete word backward

**Editing:**
- **Ctrl+T**: Transpose characters

> **Mac Note:** Alt keybindings require terminal configuration. In iTerm2: Preferences → Profiles → Keys → "Left/Right Option key" → "Esc+". Use Ctrl+Left/Right for word movement as the portable alternative.

## TreeView

A hierarchical tree view with selection and expand/collapse controls. `TreeView` is generic over the node id type.

### Basic Usage

```rust
use tui_dispatch_components::{
    Line, TreeBranchMode, TreeBranchStyle, TreeNode, TreeView, TreeViewBehavior, TreeViewProps,
    TreeViewStyle,
};

let nodes = vec![TreeNode::with_children(
    "root".to_string(),
    "Root".to_string(),
    vec![TreeNode::new("child".to_string(), "Child".to_string())],
)];
let render_node = |ctx| Line::raw(ctx.node.value.as_str());

let props = TreeViewProps {
    nodes: &nodes,
    selected_id: state.selected.as_ref(),
    expanded_ids: &state.expanded,
    is_focused: state.focused,
    style: TreeViewStyle {
        branches: TreeBranchStyle {
            mode: TreeBranchMode::Branch,
            ..Default::default()
        },
        ..Default::default()
    },
    behavior: TreeViewBehavior::default(),
    measure_node: None,
    column_padding: 0,
    on_select: Action::SelectNode,
    on_toggle: Action::ToggleNode,
    render_node: &render_node,
};

let mut tree: TreeView<String> = TreeView::new();
tree.render(frame, area, props);
```

`render_node` lets you customize the row content while TreeView still renders indentation and expand/collapse markers. Use `TreeNodeRender::tree_column_width`, `TreeNodeRender::row_width`, and `TreeNodeRender::leading_width` to align custom columns.

### Keybindings

- `j/k`, arrows: move selection
- `Left`: collapse or select parent
- `Right`: expand or select first child
- `Enter`/`Space`: toggle expand/collapse
- `g/G`, `Home/End`: jump to top/bottom

## Modal

An overlay that dims the background and renders content on top.

### Basic Usage

`Modal` renders the dimmed overlay and calls your content renderer:

```rust
use tui_dispatch_components::{
    BaseStyle, Modal, ModalBehavior, ModalProps, ModalStyle, Padding, centered_rect,
};
use ratatui::style::Color;

// Render background content first
main_view.render(frame, area, props);

if state.show_modal {
    let modal_area = centered_rect(60, 12, frame.area());
    let mut render_content = |frame: &mut Frame, content_area: Rect| {
        my_modal_content.render(frame, content_area, props);
    };

    modal.render(
        frame,
        frame.area(),
        ModalProps {
            is_open: true,
            is_focused: true,
            area: modal_area,
            style: ModalStyle {
                base: BaseStyle {
                    bg: Some(Color::Rgb(35, 35, 45)),
                    padding: Padding::all(1),
                    border: None,
                    fg: None,
                },
                ..Default::default()
            },
            behavior: ModalBehavior::default(),
            on_close: || Action::CloseModal,
            render_content: &mut render_content,
        },
    );
}
```


### Styling

```rust
use tui_dispatch_components::{ModalStyle, BorderStyle, Padding};
use ratatui::style::Color;

let style = ModalStyle {
    dim_factor: 0.5,                      // 0.0 = no dim, 1.0 = black
    border: Some(BorderStyle::default()), // Optional border
    padding: Padding::xy(2, 1),           // Inner padding
    bg: Some(Color::Rgb(30, 30, 40)),     // Background color
};
```

## Shared Style Types

All components use these common types from `tui_dispatch_components::style`:

### Padding

```rust
Padding::all(1)           // Same on all sides
Padding::xy(2, 1)         // Horizontal, vertical
Padding::new(1, 2, 1, 2)  // Top, right, bottom, left
```

### BorderStyle

```rust
BorderStyle {
    borders: Borders::ALL,
    style: Style::default().fg(Color::DarkGray),
    focused_style: Some(Style::default().fg(Color::Cyan)),
}
```

### SelectionStyle

```rust
// Default: cyan bold with "> " marker
SelectionStyle::default()

// Just a marker, no color change
SelectionStyle::marker_only("• ")

// Just style, no marker
SelectionStyle::style_only(Style::default().fg(Color::Yellow))

// User handles everything
SelectionStyle::disabled()
```

### ScrollbarStyle

```rust
// Default thumb/track styling
ScrollbarStyle::default()
```

## Prelude

For convenient imports:

```rust
use tui_dispatch_components::prelude::*;
```

This exports all components, style types, and common ratatui types (`Line`, `Color`, `Style`).
