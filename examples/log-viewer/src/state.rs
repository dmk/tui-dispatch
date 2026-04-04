use std::collections::BTreeSet;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use serde_json::{Map, Value};
use tui_dispatch::prelude::EventRoutingState;

use crate::{AppBindingContext, RouteId};

pub const DEFAULT_BUFFER_CAP: usize = 5_000;

const LEVEL_KEYS: &[&str] = &["level", "lvl", "severity"];
const SOURCE_KEYS: &[&str] = &[
    "target",
    "logger",
    "source",
    "service",
    "component",
    "module",
    "subsystem",
    "namespace",
];
const MESSAGE_KEYS: &[&str] = &["message", "msg", "event"];
const TIMESTAMP_KEYS: &[&str] = &["timestamp", "ts", "time"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Levels,
    Tags,
    Logs,
    Details,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Self::Levels => Self::Tags,
            Self::Tags => Self::Logs,
            Self::Logs => Self::Details,
            Self::Details => Self::Levels,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Levels => Self::Details,
            Self::Tags => Self::Levels,
            Self::Logs => Self::Tags,
            Self::Details => Self::Logs,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Levels => "Levels",
            Self::Tags => "Tags",
            Self::Logs => "Logs",
            Self::Details => "Details",
        }
    }

    pub fn route(self) -> RouteId {
        match self {
            Self::Levels => RouteId::Levels,
            Self::Tags => RouteId::Tags,
            Self::Logs => RouteId::Logs,
            Self::Details => RouteId::Details,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Unknown,
}

impl LogLevel {
    pub fn all() -> [Self; 6] {
        [
            Self::Trace,
            Self::Debug,
            Self::Info,
            Self::Warn,
            Self::Error,
            Self::Unknown,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Unknown => "unknown",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO ",
            Self::Warn => "WARN ",
            Self::Error => "ERROR",
            Self::Unknown => "UNKWN",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Trace => Color::Rgb(124, 143, 161),
            Self::Debug => Color::Rgb(111, 192, 201),
            Self::Info => Color::Rgb(146, 196, 125),
            Self::Warn => Color::Rgb(231, 179, 94),
            Self::Error => Color::Rgb(232, 116, 99),
            Self::Unknown => Color::Rgb(166, 166, 166),
        }
    }

    pub fn from_value(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" | "warning" => Self::Warn,
            "error" | "err" | "fatal" => Self::Error,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub seq: u64,
    pub raw_line: String,
    pub display_line: String,
    pub timestamp: Option<String>,
    pub level: LogLevel,
    pub source: String,
    pub tags: Vec<String>,
    pub message: String,
    pub payload: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub entries: Vec<LogEntry>,
    pub visible_indices: Vec<usize>,
    pub visible_lines: Vec<Line<'static>>,
    pub level_options: Vec<String>,
    pub tag_options: Vec<String>,
    pub active_levels: BTreeSet<String>,
    pub active_tags: BTreeSet<String>,
    pub selected_visible: Option<usize>,
    pub follow_mode: bool,
    pub dropped_lines: usize,
    pub focus: Focus,
    pub source_label: String,
    pub buffer_cap: usize,
    pub opened_detail_seq: Option<u64>,
    next_seq: u64,
}

impl AppState {
    pub fn new(source_label: impl Into<String>) -> Self {
        let level_options: Vec<String> = LogLevel::all()
            .into_iter()
            .map(|level| level.label().to_string())
            .collect();
        let active_levels = level_options.iter().cloned().collect();

        Self {
            entries: Vec::new(),
            visible_indices: Vec::new(),
            visible_lines: Vec::new(),
            level_options,
            tag_options: Vec::new(),
            active_levels,
            active_tags: BTreeSet::new(),
            selected_visible: None,
            follow_mode: true,
            dropped_lines: 0,
            focus: Focus::Logs,
            source_label: source_label.into(),
            buffer_cap: DEFAULT_BUFFER_CAP,
            opened_detail_seq: None,
            next_seq: 1,
        }
    }

    #[cfg(test)]
    pub fn with_buffer_cap(source_label: impl Into<String>, buffer_cap: usize) -> Self {
        let mut state = Self::new(source_label);
        state.buffer_cap = buffer_cap.max(1);
        state
    }

    pub fn selected_entry(&self) -> Option<&LogEntry> {
        let visible = self.selected_visible?;
        let index = *self.visible_indices.get(visible)?;
        self.entries.get(index)
    }

    pub fn selected_seq(&self) -> Option<u64> {
        self.selected_entry().map(|entry| entry.seq)
    }

    pub fn opened_entry(&self) -> Option<&LogEntry> {
        let seq = self.opened_detail_seq?;
        self.entries.iter().find(|entry| entry.seq == seq)
    }

    pub fn details_open(&self) -> bool {
        self.opened_detail_seq.is_some()
    }

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = if matches!(focus, Focus::Details) && !self.details_open() {
            Focus::Logs
        } else {
            focus
        };
    }

    pub fn select_visible(&mut self, index: usize) {
        if self.visible_indices.is_empty() {
            self.selected_visible = None;
            return;
        }

        self.selected_visible = Some(index.min(self.visible_indices.len().saturating_sub(1)));
        self.follow_mode = false;
    }

    pub fn toggle_follow(&mut self) {
        self.follow_mode = !self.follow_mode;
        if self.follow_mode {
            self.selected_visible = self
                .visible_indices
                .len()
                .checked_sub(1)
                .or(self.selected_visible);
        }
    }

    pub fn open_selected_details(&mut self) {
        let Some(seq) = self.selected_seq() else {
            return;
        };

        self.opened_detail_seq = Some(seq);
        self.focus = Focus::Details;
    }

    pub fn close_details(&mut self) {
        self.opened_detail_seq = None;
        if self.focus == Focus::Details {
            self.focus = Focus::Logs;
        }
    }

    pub fn focus_next(&mut self) {
        self.focus = match (self.focus, self.details_open()) {
            (Focus::Levels, _) => Focus::Tags,
            (Focus::Tags, true) => Focus::Details,
            (Focus::Tags, false) => Focus::Logs,
            (Focus::Logs, false) => Focus::Levels,
            (Focus::Logs, true) => Focus::Details,
            (Focus::Details, _) => Focus::Levels,
        };
    }

    pub fn focus_prev(&mut self) {
        self.focus = match (self.focus, self.details_open()) {
            (Focus::Levels, true) => Focus::Details,
            (Focus::Levels, false) => Focus::Logs,
            (Focus::Tags, _) => Focus::Levels,
            (Focus::Logs, _) => Focus::Tags,
            (Focus::Details, true) => Focus::Tags,
            (Focus::Details, false) => Focus::Logs,
        };
    }

    pub fn toggle_level(&mut self, level: &str) {
        toggle_filter(&mut self.active_levels, level);
        let selected_seq = self.selected_seq();
        self.rebuild_visible(selected_seq);

        if self
            .opened_detail_seq
            .is_some_and(|seq| !self.entries.iter().any(|entry| entry.seq == seq))
        {
            self.close_details();
        }
    }

    pub fn toggle_tag(&mut self, tag: &str) {
        toggle_filter(&mut self.active_tags, tag);
        let selected_seq = self.selected_seq();
        self.rebuild_visible(selected_seq);
    }

    pub fn append_lines(&mut self, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }

        let selected_seq = self.selected_seq();

        for raw_line in lines {
            let entry = parse_log_entry(self.next_seq, raw_line);
            self.next_seq = self.next_seq.saturating_add(1);

            for tag in &entry.tags {
                if !self.tag_options.contains(tag) {
                    self.tag_options.push(tag.clone());
                    self.tag_options.sort();
                    self.active_tags.insert(tag.clone());
                }
            }

            self.entries.push(entry);
        }

        if self.entries.len() > self.buffer_cap {
            let overflow = self.entries.len() - self.buffer_cap;
            self.entries.drain(0..overflow);
            self.dropped_lines += overflow;
        }

        self.rebuild_visible(selected_seq);
    }

    pub fn visible_count(&self) -> usize {
        self.visible_indices.len()
    }

    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    fn rebuild_visible(&mut self, selected_seq: Option<u64>) {
        self.visible_indices = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| self.matches_filters(entry).then_some(index))
            .collect();

        self.visible_lines = self
            .visible_indices
            .iter()
            .map(|&index| render_log_summary(&self.entries[index]))
            .collect();

        self.selected_visible = if self.visible_indices.is_empty() {
            None
        } else if self.follow_mode {
            Some(self.visible_indices.len().saturating_sub(1))
        } else if let Some(seq) = selected_seq {
            self.visible_indices
                .iter()
                .position(|&index| self.entries[index].seq == seq)
                .or_else(|| {
                    self.selected_visible
                        .map(|index| index.min(self.visible_indices.len().saturating_sub(1)))
                })
                .or(Some(0))
        } else {
            self.selected_visible
                .map(|index| index.min(self.visible_indices.len().saturating_sub(1)))
                .or(Some(0))
        };
    }

    fn matches_filters(&self, entry: &LogEntry) -> bool {
        self.active_levels.contains(entry.level.label())
            && entry.tags.iter().all(|tag| self.active_tags.contains(tag))
    }
}

impl EventRoutingState<RouteId, AppBindingContext> for AppState {
    fn focused(&self) -> Option<RouteId> {
        Some(self.focus.route())
    }

    fn modal(&self) -> Option<RouteId> {
        None
    }

    fn binding_context(&self, id: RouteId) -> AppBindingContext {
        match id {
            RouteId::Levels | RouteId::Tags => AppBindingContext::Filter,
            RouteId::Logs => AppBindingContext::Logs,
            RouteId::Details => AppBindingContext::Details,
        }
    }

    fn default_context(&self) -> AppBindingContext {
        AppBindingContext::Logs
    }
}

impl tui_dispatch_debug::debug::DebugState for AppState {
    fn debug_sections(&self) -> Vec<tui_dispatch_debug::debug::DebugSection> {
        use tui_dispatch_debug::debug::DebugSection;
        vec![
            DebugSection::new("Buffer")
                .entry("entries", self.entries.len().to_string())
                .entry("visible", self.visible_indices.len().to_string())
                .entry("dropped", self.dropped_lines.to_string())
                .entry("capacity", self.buffer_cap.to_string()),
            DebugSection::new("Filters")
                .entry("active_levels", format!("{:?}", self.active_levels))
                .entry("active_tags", format!("{:?}", self.active_tags)),
            DebugSection::new("UI")
                .entry("focus", format!("{:?}", self.focus))
                .entry("follow_mode", self.follow_mode.to_string())
                .entry(
                    "selected",
                    self.selected_visible
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "none".into()),
                )
                .entry("details_open", self.details_open().to_string()),
        ]
    }
}

fn toggle_filter(active: &mut BTreeSet<String>, value: &str) {
    if !active.remove(value) {
        active.insert(value.to_string());
    }
}

pub fn parse_log_entry(seq: u64, raw_line: String) -> LogEntry {
    let display_line = strip_ansi(&raw_line);

    if let Ok(value) = serde_json::from_str::<Value>(&display_line) {
        return parse_json_entry(seq, raw_line, display_line, value);
    }

    LogEntry {
        seq,
        raw_line,
        display_line: display_line.clone(),
        timestamp: None,
        level: LogLevel::Unknown,
        source: "plain".to_string(),
        tags: vec!["format:plain".to_string()],
        message: display_line,
        payload: None,
    }
}

fn parse_json_entry(seq: u64, raw_line: String, display_line: String, value: Value) -> LogEntry {
    if let Some(object) = value.as_object() {
        let timestamp = extract_string(object, TIMESTAMP_KEYS);
        let level = extract_string(object, LEVEL_KEYS)
            .map(|value| LogLevel::from_value(&value))
            .unwrap_or(LogLevel::Unknown);
        let source = extract_source(object);
        let tags = extract_tags(object);
        let message = extract_string(object, MESSAGE_KEYS).unwrap_or_else(|| display_line.clone());

        return LogEntry {
            seq,
            raw_line,
            display_line,
            timestamp,
            level,
            source,
            tags,
            message,
            payload: Some(value),
        };
    }

    let message = compact_json(&value);

    LogEntry {
        seq,
        raw_line,
        display_line,
        timestamp: None,
        level: LogLevel::Unknown,
        source: "json".to_string(),
        tags: vec!["format:json".to_string()],
        message,
        payload: Some(value),
    }
}

fn extract_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key))
        .map(compact_json)
        .filter(|value| !value.is_empty())
}

fn extract_source(object: &Map<String, Value>) -> String {
    let service = extract_string(object, &["service", "app", "application"]);
    let scope = extract_string(object, SOURCE_KEYS)
        .or_else(|| extract_nested_string(object, &[&["span", "name"], &["log", "logger"]]));

    match (service, scope) {
        (Some(service), Some(scope)) if service != scope => format!("{service}/{scope}"),
        (_, Some(scope)) => scope,
        (Some(service), None) => service,
        (None, None) => "json".to_string(),
    }
}

fn extract_tags(object: &Map<String, Value>) -> Vec<String> {
    let mut tags = BTreeSet::new();

    for (key, value) in object {
        if should_skip_tag_key(key) {
            continue;
        }

        if matches!(key.as_str(), "tags" | "labels" | "fields") {
            collect_custom_tags(key, value, &mut tags);
            continue;
        }

        if let Some(tag) = scalar_tag(key, value) {
            tags.insert(tag);
        }
    }

    if tags.is_empty() {
        tags.insert("format:json".to_string());
    }

    tags.into_iter().collect()
}

fn should_skip_tag_key(key: &str) -> bool {
    LEVEL_KEYS.contains(&key)
        || MESSAGE_KEYS.contains(&key)
        || TIMESTAMP_KEYS.contains(&key)
        || matches!(
            key,
            "payload" | "raw" | "stack" | "trace" | "error" | "exception"
        )
        || key == "id"
        || key.ends_with("_id")
}

fn scalar_tag(key: &str, value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.is_empty() => Some(format!("{key}:{value}")),
        Value::Bool(value) => Some(format!("{key}:{value}")),
        Value::Number(value) => Some(format!("{key}:{value}")),
        _ => None,
    }
}

fn collect_custom_tags(prefix: &str, value: &Value, tags: &mut BTreeSet<String>) {
    match value {
        Value::String(value) if !value.is_empty() => {
            tags.insert(value.clone());
        }
        Value::Array(values) => {
            for value in values {
                collect_custom_tags(prefix, value, tags);
            }
        }
        Value::Object(object) => {
            for (key, value) in object {
                let tag_key = prefixed_tag_key(prefix, key);
                match value {
                    Value::String(_) | Value::Bool(_) | Value::Number(_) => {
                        if let Some(tag) = scalar_tag(&tag_key, value) {
                            tags.insert(tag);
                        }
                    }
                    Value::Array(_) | Value::Object(_) => {
                        collect_custom_tags(&tag_key, value, tags);
                    }
                    Value::Null => {}
                }
            }
        }
        _ => {}
    }
}

fn prefixed_tag_key(prefix: &str, key: &str) -> String {
    if matches!(prefix, "tags" | "labels" | "fields") {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

fn extract_nested_string(object: &Map<String, Value>, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut current: &Value = object.get(path.first().copied()?)?;
        for key in &path[1..] {
            current = current.get(*key)?;
        }
        let value = compact_json(current);
        (!value.is_empty()).then_some(value)
    })
}

fn compact_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

pub fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            output.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' && matches!(chars.peek(), Some('\\')) {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    output
}

fn render_log_summary(entry: &LogEntry) -> Line<'static> {
    let mut spans = Vec::new();

    if let Some(timestamp) = &entry.timestamp {
        spans.push(Span::styled(
            format!("{timestamp} "),
            Style::default().fg(Color::Rgb(124, 132, 150)),
        ));
    }

    spans.push(Span::styled(
        format!("{} ", entry.level.badge()),
        Style::default()
            .fg(entry.level.color())
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        format!("{} ", entry.source),
        Style::default().fg(Color::Rgb(104, 174, 195)),
    ));
    spans.push(Span::styled(
        entry.message.clone(),
        Style::default().fg(Color::Rgb(229, 229, 227)),
    ));

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_with_common_fields() {
        let entry = parse_log_entry(
            7,
            r#"{"timestamp":"2026-03-14T12:00:00Z","level":"warn","target":"api","message":"slow query"}"#.to_string(),
        );

        assert_eq!(entry.seq, 7);
        assert_eq!(entry.timestamp.as_deref(), Some("2026-03-14T12:00:00Z"));
        assert_eq!(entry.level, LogLevel::Warn);
        assert_eq!(entry.source, "api");
        assert_eq!(entry.message, "slow query");
        assert!(entry.payload.is_some());
    }

    #[test]
    fn parses_json_without_common_fields() {
        let entry = parse_log_entry(
            1,
            r#"{"kind":"deploy","region":"iad","ok":true}"#.to_string(),
        );

        assert_eq!(entry.level, LogLevel::Unknown);
        assert_eq!(entry.source, "json");
        assert!(entry.message.contains("\"kind\":\"deploy\""));
        assert!(entry.payload.is_some());
    }

    #[test]
    fn builds_composite_source_from_service_and_target() {
        let entry = parse_log_entry(
            4,
            r#"{"service":"billing","target":"worker","message":"ok"}"#.to_string(),
        );

        assert_eq!(entry.source, "billing/worker");
    }

    #[test]
    fn falls_back_to_extended_source_fields() {
        let entry = parse_log_entry(5, r#"{"module":"sync.jobs","message":"ok"}"#.to_string());

        assert_eq!(entry.source, "sync.jobs");
    }

    #[test]
    fn extracts_custom_tags_from_tag_object_and_array() {
        let entry = parse_log_entry(
            6,
            r#"{"message":"ok","tags":{"env":"prod","team":"search"},"labels":["canary","blue"]}"#
                .to_string(),
        );

        assert!(entry.tags.contains(&"env:prod".to_string()));
        assert!(entry.tags.contains(&"team:search".to_string()));
        assert!(entry.tags.contains(&"canary".to_string()));
        assert!(entry.tags.contains(&"blue".to_string()));
    }

    #[test]
    fn falls_back_for_plain_text() {
        let entry = parse_log_entry(2, "plain line".to_string());

        assert_eq!(entry.level, LogLevel::Unknown);
        assert_eq!(entry.source, "plain");
        assert_eq!(entry.message, "plain line");
        assert!(entry.payload.is_none());
    }

    #[test]
    fn strips_ansi_sequences_from_display_text() {
        let entry = parse_log_entry(3, "\u{1b}[31mERROR\u{1b}[0m boom".to_string());

        assert_eq!(entry.display_line, "ERROR boom");
        assert_eq!(entry.message, "ERROR boom");
    }
}
