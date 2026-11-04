//! Application startup, executor configuration, and component lifecycle management.
//!
//! [`Runtime`] starts an application; [`Components`] also works inside an existing Tokio
//! runtime. Command-line parsing is an optional adapter in the `cli` module.
extern crate self as eva_rt;

pub mod cfg;
#[cfg(feature = "cli")]
pub mod cli;
pub mod components;
pub mod executor;
pub mod logging;
mod runtime;
pub mod signal;

pub use components::Components;
pub use runtime::Runtime;

pub use eva_core::data;
#[doc(hidden)]
pub mod _priv {
    pub use eva_core::_priv::*;
}
