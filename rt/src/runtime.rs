use eyre::Context as _;

use crate::{
    cfg::{self, ComponentConfigs, IsComponentConfig},
    executor,
};

/// Configures and starts an application. CLI parsing is an optional adapter.
pub struct Runtime {
    cfg: cfg::Builder,
}

impl Default for Runtime {
    fn default() -> Self {
        let cfg = cfg::Builder::default().optional::<executor::Config>("system.rt");
        #[cfg(feature = "logforth")]
        let cfg = cfg.optional::<crate::logging::logforth::Config>("system.logging");
        Self { cfg }
    }
}

impl Runtime {
    pub fn new() -> Self {
        Self::default()
    }

    /// Customize configuration sources and registrations before startup.
    pub fn configure(mut self, configure: impl FnOnce(cfg::Builder) -> cfg::Builder) -> Self {
        self.cfg = configure(self.cfg);
        self
    }

    #[track_caller]
    pub fn optional<T: IsComponentConfig + Default>(self, section: &str) -> Self {
        self.configure(|cfg| cfg.optional::<T>(section))
    }

    #[track_caller]
    pub fn require<T: IsComponentConfig>(self, section: &str) -> Self {
        self.configure(|cfg| cfg.require::<T>(section))
    }

    /// Set a final typed configuration value, including executor or logging settings.
    pub fn config<T: IsComponentConfig>(self, section: &str, value: T) -> Self {
        self.configure(|cfg| cfg.set(section, value))
    }

    #[cfg(feature = "figment")]
    pub fn config_file(mut self, path: impl AsRef<std::path::Path>) -> eyre::Result<Self> {
        self.cfg = self.cfg.file(path)?;
        Ok(self)
    }

    #[cfg(feature = "figment")]
    pub fn env_prefix(self, prefix: &str) -> Self {
        self.configure(|cfg| cfg.env_prefix(prefix))
    }

    /// Initialize configuration, logging (when enabled), and the executor, then run `app`.
    pub fn run(
        self,
        app: impl AsyncFnOnce(ComponentConfigs) -> eyre::Result<()>,
    ) -> eyre::Result<()> {
        let configs = self.cfg.build()?;
        #[cfg(feature = "logforth")]
        crate::logging::logforth::setup(configs.get()).wrap_err("failed to initialize logging")?;
        let executor = configs
            .get::<executor::Config>()
            .build()
            .wrap_err("failed to create Tokio runtime")?;
        executor.block_on(app(configs))
    }
}
