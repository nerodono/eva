//! Optional clap adapter. Configuration and executor construction live outside CLI.
use std::{ffi::OsString, path::PathBuf};

use clap::Parser;

use crate::{Runtime, cfg::ComponentConfigs};

#[derive(Debug, Clone, Copy, clap::Args)]
pub struct NoArgs {}

#[derive(Debug, Parser)]
// The flattened application type may also be named Args.
#[group(skip)]
pub struct Args<A: clap::Args> {
    /// Configuration files, merged in the given order.
    #[arg(long, short)]
    pub config: Vec<PathBuf>,
    #[command(flatten)]
    pub args: A,
}

impl<A: clap::Args> Args<A> {
    /// Parse explicitly supplied arguments without exiting the process.
    pub fn try_from_iter(
        args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
    ) -> Result<Self, clap::Error> {
        Self::try_parse_from(args)
    }

    /// Apply CLI-selected sources while leaving startup to [`Runtime`].
    pub fn apply(self, mut runtime: Runtime) -> eyre::Result<(Runtime, A)> {
        for path in self.config {
            runtime = runtime.config_file(path)?;
        }
        Ok((runtime, self.args))
    }
}

/// Parse process arguments and launch an application with the given runtime settings.
pub fn run<A: clap::Args>(
    runtime: Runtime,
    app: impl AsyncFnOnce(A, ComponentConfigs) -> eyre::Result<()>,
) -> eyre::Result<()> {
    let (runtime, args) = Args::<A>::parse().apply(runtime)?;
    runtime.run(async move |configs| app(args, configs).await)
}
