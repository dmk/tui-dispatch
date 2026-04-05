use std::{
    io::{self, Write},
    time::Duration,
};

use ratatui::{
    backend::TestBackend,
    buffer::Buffer,
    style::{Color as TuiColor, Modifier},
    Frame, Terminal,
};
use tinycrossterm::{
    cursor::MoveTo,
    event::{self, Event},
    execute, queue,
    style::{
        Attribute, Color as CtColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
        SetForegroundColor,
    },
    terminal::{Clear, ClearType},
};

pub fn viewport_size_from_args() -> (u16, u16) {
    let parsed: Vec<u16> = std::env::args()
        .filter_map(|arg| arg.parse::<u16>().ok())
        .collect();
    let cols = parsed
        .get(parsed.len().saturating_sub(2))
        .copied()
        .unwrap_or(80)
        .max(1);
    let rows = parsed.last().copied().unwrap_or(24).max(1);
    (cols, rows)
}

pub fn draw_frame<F>(cols: u16, rows: u16, render: F) -> io::Result<()>
where
    F: FnOnce(&mut Frame),
{
    let mut terminal = Terminal::new(TestBackend::new(cols, rows))?;
    terminal.draw(render)?;
    let buffer = terminal.backend().buffer();
    render_to_ansi(io::stdout().lock(), buffer)
}

pub fn drain_events() -> io::Result<Vec<Event>> {
    let mut out = Vec::new();
    loop {
        match event::poll(Duration::ZERO) {
            Ok(true) => match event::read() {
                Ok(event) => out.push(event),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => return Err(err),
            },
            Ok(false) => break,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
            Err(err) => return Err(err),
        }
    }
    Ok(out)
}

fn render_to_ansi(mut out: impl Write, buffer: &Buffer) -> io::Result<()> {
    execute!(out, Clear(ClearType::All), MoveTo(0, 0))?;

    let area = buffer.area;
    let mut current_fg = CtColor::Reset;
    let mut current_bg = CtColor::Reset;
    let mut current_mod = Modifier::empty();

    for y in 0..area.height {
        queue!(out, MoveTo(0, y))?;

        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            let style = cell.style();
            let fg = to_ct_color(style.fg.unwrap_or(TuiColor::Reset));
            let bg = to_ct_color(style.bg.unwrap_or(TuiColor::Reset));
            let modifier = style.add_modifier;

            if modifier != current_mod {
                queue!(out, SetAttribute(Attribute::Reset))?;
                current_fg = CtColor::Reset;
                current_bg = CtColor::Reset;
                apply_modifier(&mut out, modifier)?;
                current_mod = modifier;
            }

            if fg != current_fg {
                queue!(out, SetForegroundColor(fg))?;
                current_fg = fg;
            }

            if bg != current_bg {
                queue!(out, SetBackgroundColor(bg))?;
                current_bg = bg;
            }

            queue!(out, Print(cell.symbol()))?;
        }
    }

    queue!(
        out,
        ResetColor,
        SetAttribute(Attribute::Reset),
        MoveTo(0, area.height.saturating_sub(1))
    )?;

    out.flush()
}

fn apply_modifier(mut out: &mut impl Write, modifier: Modifier) -> io::Result<()> {
    if modifier.contains(Modifier::BOLD) {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    if modifier.contains(Modifier::DIM) {
        queue!(out, SetAttribute(Attribute::Dim))?;
    }
    if modifier.contains(Modifier::ITALIC) {
        queue!(out, SetAttribute(Attribute::Italic))?;
    }
    if modifier.contains(Modifier::UNDERLINED) {
        queue!(out, SetAttribute(Attribute::Underlined))?;
    }
    if modifier.contains(Modifier::SLOW_BLINK) {
        queue!(out, SetAttribute(Attribute::SlowBlink))?;
    }
    if modifier.contains(Modifier::RAPID_BLINK) {
        queue!(out, SetAttribute(Attribute::RapidBlink))?;
    }
    if modifier.contains(Modifier::REVERSED) {
        queue!(out, SetAttribute(Attribute::Reverse))?;
    }
    if modifier.contains(Modifier::HIDDEN) {
        queue!(out, SetAttribute(Attribute::Hidden))?;
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        queue!(out, SetAttribute(Attribute::CrossedOut))?;
    }
    Ok(())
}

fn to_ct_color(color: TuiColor) -> CtColor {
    match color {
        TuiColor::Reset => CtColor::Reset,
        TuiColor::Black => CtColor::Black,
        TuiColor::Red => CtColor::DarkRed,
        TuiColor::Green => CtColor::DarkGreen,
        TuiColor::Yellow => CtColor::DarkYellow,
        TuiColor::Blue => CtColor::DarkBlue,
        TuiColor::Magenta => CtColor::DarkMagenta,
        TuiColor::Cyan => CtColor::DarkCyan,
        TuiColor::Gray => CtColor::Grey,
        TuiColor::DarkGray => CtColor::DarkGrey,
        TuiColor::LightRed => CtColor::Red,
        TuiColor::LightGreen => CtColor::Green,
        TuiColor::LightYellow => CtColor::Yellow,
        TuiColor::LightBlue => CtColor::Blue,
        TuiColor::LightMagenta => CtColor::Magenta,
        TuiColor::LightCyan => CtColor::Cyan,
        TuiColor::White => CtColor::White,
        TuiColor::Indexed(i) => CtColor::AnsiValue(i),
        TuiColor::Rgb(r, g, b) => CtColor::Rgb { r, g, b },
    }
}
