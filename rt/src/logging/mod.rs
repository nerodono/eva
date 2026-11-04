pub use log::{debug, error, info, log, log_enabled, trace, warn};

#[cfg(feature = "logforth")]
pub mod logforth;
