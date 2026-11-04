//! Command-line arguments for [`crate::Runtime`].
use std::{ffi::OsString, path::PathBuf};

use clap::Parser;

use crate::cfg;

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

    /// Add CLI-selected files to the configuration sources and return application arguments.
    pub fn apply(self, mut cfg: cfg::Builder) -> eyre::Result<(cfg::Builder, A)> {
        for path in self.config {
            cfg = cfg.file(path)?;
        }
        Ok((cfg, self.args))
    }
}
