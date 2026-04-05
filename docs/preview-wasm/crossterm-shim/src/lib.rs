use std::io;

pub use tinycrossterm::{execute, queue, Command};

pub mod event {
    pub use tinycrossterm::event::*;
}

pub mod cursor {
    use std::io;

    pub use tinycrossterm::cursor::{Hide, MoveTo, SetCursorStyle, Show};

    pub fn position() -> io::Result<(u16, u16)> {
        Ok((0, 0))
    }
}

pub mod terminal {
    use std::io;

    pub use tinycrossterm::terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowSize {
        pub columns: u16,
        pub rows: u16,
        pub width: u16,
        pub height: u16,
    }

    pub fn window_size() -> io::Result<WindowSize> {
        let (columns, rows) = size()?;
        Ok(WindowSize {
            columns,
            rows,
            width: 0,
            height: 0,
        })
    }
}

pub mod style {
    use std::io::{self, Write};

    pub use tinycrossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};

    use crate::Command;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[allow(clippy::enum_variant_names)]
    pub enum Attribute {
        Reset,
        Bold,
        Dim,
        Italic,
        Underlined,
        SlowBlink,
        RapidBlink,
        Reverse,
        Hidden,
        CrossedOut,
        Fraktur,
        NoBold,
        NormalIntensity,
        NoItalic,
        NoUnderline,
        NoBlink,
        NoReverse,
        NoHidden,
        NotCrossedOut,
        DoubleUnderlined,
        Undercurled,
        Underdotted,
        Underdashed,
    }

    impl Attribute {
        fn sgr(self) -> &'static str {
            match self {
                Attribute::Reset => "0",
                Attribute::Bold => "1",
                Attribute::Dim => "2",
                Attribute::Italic => "3",
                Attribute::Underlined => "4",
                Attribute::SlowBlink => "5",
                Attribute::RapidBlink => "6",
                Attribute::Reverse => "7",
                Attribute::Hidden => "8",
                Attribute::CrossedOut => "9",
                Attribute::Fraktur => "20",
                Attribute::NoBold | Attribute::NormalIntensity => "22",
                Attribute::NoItalic => "23",
                Attribute::NoUnderline => "24",
                Attribute::NoBlink => "25",
                Attribute::NoReverse => "27",
                Attribute::NoHidden => "28",
                Attribute::NotCrossedOut => "29",
                Attribute::DoubleUnderlined => "21",
                Attribute::Undercurled => "4",
                Attribute::Underdotted => "4",
                Attribute::Underdashed => "4",
            }
        }

        const fn bit(self) -> u128 {
            match self {
                Attribute::Reset => 1 << 0,
                Attribute::Bold => 1 << 1,
                Attribute::Dim => 1 << 2,
                Attribute::Italic => 1 << 3,
                Attribute::Underlined => 1 << 4,
                Attribute::SlowBlink => 1 << 5,
                Attribute::RapidBlink => 1 << 6,
                Attribute::Reverse => 1 << 7,
                Attribute::Hidden => 1 << 8,
                Attribute::CrossedOut => 1 << 9,
                Attribute::Fraktur => 1 << 10,
                Attribute::NoBold => 1 << 11,
                Attribute::NormalIntensity => 1 << 12,
                Attribute::NoItalic => 1 << 13,
                Attribute::NoUnderline => 1 << 14,
                Attribute::NoBlink => 1 << 15,
                Attribute::NoReverse => 1 << 16,
                Attribute::NoHidden => 1 << 17,
                Attribute::NotCrossedOut => 1 << 18,
                Attribute::DoubleUnderlined => 1 << 19,
                Attribute::Undercurled => 1 << 20,
                Attribute::Underdotted => 1 << 21,
                Attribute::Underdashed => 1 << 22,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Attributes(u128);

    impl Attributes {
        pub const fn has(self, attr: Attribute) -> bool {
            (self.0 & attr.bit()) != 0
        }

        pub fn set(&mut self, attr: Attribute) {
            self.0 |= attr.bit();
        }

        pub fn unset(&mut self, attr: Attribute) {
            self.0 &= !attr.bit();
        }
    }

    impl From<Attribute> for Attributes {
        fn from(value: Attribute) -> Self {
            Self(value.bit())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Colors {
        pub foreground: Color,
        pub background: Color,
    }

    impl Colors {
        pub const fn new(foreground: Color, background: Color) -> Self {
            Self {
                foreground,
                background,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ContentStyle {
        pub foreground_color: Option<Color>,
        pub background_color: Option<Color>,
        pub underline_color: Option<Color>,
        pub attributes: Attributes,
    }

    impl Default for ContentStyle {
        fn default() -> Self {
            Self {
                foreground_color: None,
                background_color: None,
                underline_color: None,
                attributes: Attributes::default(),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SetAttribute(pub Attribute);

    impl Command for SetAttribute {
        fn write_ansi(&self, w: &mut impl Write) -> io::Result<()> {
            write!(w, "\x1b[{}m", self.0.sgr())
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SetColors(pub Colors);

    impl Command for SetColors {
        fn write_ansi(&self, w: &mut impl Write) -> io::Result<()> {
            SetForegroundColor(self.0.foreground).write_ansi(w)?;
            SetBackgroundColor(self.0.background).write_ansi(w)
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SetUnderlineColor(pub Color);

    impl Command for SetUnderlineColor {
        fn write_ansi(&self, _w: &mut impl Write) -> io::Result<()> {
            Ok(())
        }
    }
}

pub type Result<T> = io::Result<T>;
