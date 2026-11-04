//! Eva: foundational types, handlers, and optional application runtime.
pub use eva_core::*;

#[cfg(feature = "handlers")]
pub use eva_handlers as handlers;
#[cfg(feature = "handlers")]
pub use eva_handlers::{endpoint, export, if_flag, service};

#[cfg(feature = "rt")]
pub use eva_rt as rt;

#[doc(hidden)]
pub mod _priv {
    pub use eva_core::_priv::*;
    #[cfg(feature = "handlers")]
    pub use eva_handlers::_priv::{CallStep, Endpoint, check_flags, fastrace};
}
