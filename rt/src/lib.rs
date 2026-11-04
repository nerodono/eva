//! Application startup, executor configuration, and component lifecycle management.
//!
//! With the `cli` feature, `Runtime` parses command-line arguments and starts an application.
//! [`Components`] works independently inside an existing Tokio runtime.
extern crate self as eva_rt;

pub mod cfg;
#[cfg(feature = "cli")]
pub mod cli;
pub mod components;
pub mod executor;
pub mod logging;
#[cfg(feature = "cli")]
mod runtime;
pub mod signal;

pub use components::Components;
#[cfg(feature = "cli")]
pub use runtime::Runtime;

pub use eva_core::data;
#[doc(hidden)]
pub mod _priv {
    pub use eva_core::_priv::*;
}
