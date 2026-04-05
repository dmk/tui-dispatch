pub use tui_dispatch_core::*;

pub mod prelude {
    pub use tui_dispatch_core::prelude::*;
}

pub mod debug {
    use std::fmt::Debug;

    pub fn debug_string<T: Debug>(value: &T) -> String {
        format!("{value:?}")
    }

    pub fn debug_string_pretty<T: Debug>(value: &T) -> String {
        format!("{value:#?}")
    }
}
