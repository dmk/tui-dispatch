//! Core traits and types for tui-dispatch
//!
//! This crate provides the foundational abstractions for building TUI applications
//! with centralized state management.

pub mod action;
pub mod component;
pub mod event;

pub use action::Action;
pub use component::Component;
pub use event::{Event, EventContext, EventKind, EventType};

// Re-export ratatui types for convenience
pub use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    Frame,
};
