//! Runtime helpers: core loops plus event-bus routing.

mod bus;
mod core;
#[cfg(feature = "testing-time")]
pub(crate) use self::core::draw_frame;

#[doc(hidden)]
pub use self::bus::EventBusRouting;
#[cfg(feature = "debug")]
pub use self::core::DebugAdapter;
pub use self::core::{
    Direct, DispatchErrorPolicy, EffectContext, PollerConfig, RenderContext, Runtime, RuntimeStore,
};
