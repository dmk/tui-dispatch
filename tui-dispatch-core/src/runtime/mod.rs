//! Runtime helpers: core loops plus event-bus routing.

mod bus;
mod core;

#[doc(hidden)]
pub use self::bus::EventBusRouting;
#[cfg(feature = "debug")]
pub use self::core::DebugAdapter;
pub use self::core::{
    Direct, DispatchErrorPolicy, EffectContext, PollerConfig, RenderContext, Runtime, RuntimeStore,
};
