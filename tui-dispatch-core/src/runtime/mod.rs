//! Runtime helpers: core loops plus the Layer 1 bus-composition wrappers.
//!
//! `core` owns the event/action/render loop for `DispatchRuntime` and
//! `EffectRuntime`. `bus` adds the thin Layer 1 wrappers returned by
//! `with_event_bus(...)` that pair a runtime with an `EventBus` +
//! `Keybindings`.

mod bus;
mod core;

pub use self::bus::{BusDispatchRuntime, BusEffectRuntime};
#[cfg(feature = "debug")]
pub use self::core::{DebugAdapter, DebugHooks};
pub use self::core::{
    DispatchErrorPolicy, DispatchRuntime, DispatchStore, EffectContext, EffectDispatchStore,
    EffectRuntime, PollerConfig, RenderContext,
};
