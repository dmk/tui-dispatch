# Pre-built Components

The `tui-dispatch-components` crate provides reusable UI components that integrate with tui-dispatch patterns. All components implement the `Component<A>` trait.

## Installation

```toml
[dependencies]
tui-dispatch-components = "0.4"
```

## Available Components

- **SelectList** - Scrollable selection list with keyboard navigation
- **TextInput** - Single-line text input with cursor
- **Modal** - Overlay with dimmed background

## Design Philosophy

Components are designed around three principles:

1. **Unified styling** - Each component style implements `ComponentStyle` trait with common fields
2. **User-controlled rendering** - For lists, users provide pre-styled `Line` items
3. **Behavior configuration** - Runtime behavior (scrollbar, wrap-around) is configurable

### ComponentStyle Trait

All component styles implement `ComponentStyle`, ensuring consistent access to common fields:

```rust
pub trait ComponentStyle {
    fn border(&self) -> Option<&BorderStyle>;
    fn padding(&self) -> &Padding;
    fn bg(&self) -> Option<Color>;
}
```

This enables generic code to work with any component style.

## SelectList

A scrollable list with keyboard navigation (j/k, arrows, g/G, Enter).

### Basic Usage

```rust
use tui_dispatch_components::{SelectList, SelectListProps, ListStyle, ListBehavior, Line};

let items: Vec<Line> = vec![
    Line::raw("Option 1"),
    Line::raw("Option 2"),
    Line::raw("Option 3"),
];

// Using the helper constructor (sets count, is_focused, style, behavior to defaults)
let props = SelectListProps::new(&items, state.selected, |idx| Action::Select(idx));

list.render(frame, area, props);
```

Or with full control:

```rust
let props = SelectListProps {
    items: &items,
    count: items.len(),
    selected: state.selected,
    is_focused: true,
    style: ListStyle::default(),
    behavior: ListBehavior::default(),
    on_select: |idx| Action::Select(idx),
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
use tui_dispatch_components::{ListStyle, Padding, BorderStyle, SelectionStyle};
use ratatui::style::{Color, Style};

let style = ListStyle {
    border: Some(BorderStyle::default()),
    padding: Padding::xy(1, 0),
    bg: Some(Color::Rgb(30, 30, 40)),
    selection: SelectionStyle {
        style: Some(Style::default().fg(Color::Cyan)),
        marker: Some("> "),
        disabled: false,
    },
};
```

#### Disabling Selection Styling

When you want full control over how selected items look:

```rust
let style = ListStyle {
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
let behavior = ListBehavior {
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

## TextInput

A single-line text input with cursor movement and editing.

### Basic Usage

```rust
use tui_dispatch_components::{TextInput, TextInputProps, InputStyle};

let props = TextInputProps {
    value: &state.input_value,
    placeholder: "Type here...",
    is_focused: true,
    style: InputStyle::default(),
    on_change: |s| Action::InputChange(s),
    on_submit: |s| Action::InputSubmit(s),
};

input.render(frame, area, props);
```

### Styling

```rust
use tui_dispatch_components::{InputStyle, Padding, BorderStyle};
use ratatui::style::{Color, Style};

let style = InputStyle {
    border: Some(BorderStyle::default()),
    padding: Padding::xy(1, 0),
    bg: Some(Color::Rgb(50, 50, 60)),
    fg: Some(Color::White),
    placeholder_style: Some(Style::default().fg(Color::DarkGray)),
    cursor_style: None,
};
```

### Keyboard Handling

- **Characters**: Insert at cursor
- **Backspace/Delete**: Remove characters
- **Left/Right/Home/End**: Move cursor
- **Ctrl+A/E**: Move to start/end
- **Ctrl+U**: Clear line
- **Enter**: Submit

## Modal

An overlay that dims the background and renders content on top.

### Basic Usage

`render_modal` returns the inner content area (after applying border and padding):

```rust
use tui_dispatch_components::{render_modal, centered_rect, ModalStyle, Padding};
use ratatui::style::Color;

// Render background content first
main_view.render(frame, area, props);

// Then render modal on top
if state.show_modal {
    let modal_area = centered_rect(60, 12, frame.area());
    let content_area = render_modal(frame, modal_area, &ModalStyle {
        bg: Some(Color::Rgb(35, 35, 45)),
        padding: Padding::all(1),
        ..Default::default()
    });

    // Render modal content in the returned content area
    modal_content.render(frame, content_area, modal_props);
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

## Prelude

For convenient imports:

```rust
use tui_dispatch_components::prelude::*;
```

This exports all components, style types, and common ratatui types (`Line`, `Color`, `Style`).
