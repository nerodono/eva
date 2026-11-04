use clap::Parser as _;
use eyre::Context as _;

use crate::{
    cfg::{self, ComponentConfigs},
    cli, executor,
};

/// Configures and starts a command-line application.
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

    /// Parse CLI arguments, initialize configuration, logging, and the executor, then run `app`.
    pub fn run<A: clap::Args>(
        self,
        app: impl AsyncFnOnce(A, ComponentConfigs) -> eyre::Result<()>,
    ) -> eyre::Result<()> {
        let (cfg, args) = cli::Args::<A>::parse().apply(self.cfg)?;
        let configs = cfg.build()?;
        #[cfg(feature = "logforth")]
        crate::logging::logforth::setup(configs.get()).wrap_err("failed to initialize logging")?;
        let executor = configs
            .get::<executor::Config>()
            .build()
            .wrap_err("failed to create Tokio runtime")?;
        executor.block_on(app(args, configs))
    }
}
