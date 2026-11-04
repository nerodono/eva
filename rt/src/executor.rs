use std::num::NonZeroUsize;

use crate::data;

#[data(crate = crate)]
#[derive(Default)]
#[serde(default)]
pub struct Config {
    pub flavor: Flavor,
}

#[data(crate = crate, copy)]
#[derive(Default)]
pub enum ThreadsCount {
    #[default]
    ThreadPerCpu,
    #[serde(untagged)]
    Specific(NonZeroUsize),
}

#[data(crate = crate, copy)]
pub enum Flavor {
    SingleThreaded,
    #[serde(rename = "worker_threads")]
    MultiThreaded {
        #[serde(default)]
        threads: ThreadsCount,
    },
}

impl Default for Flavor {
    fn default() -> Self {
        Self::MultiThreaded {
            threads: ThreadsCount::default(),
        }
    }
}

impl Config {
    /// Build the Tokio executor without parsing arguments or initializing logging.
    pub fn build(&self) -> std::io::Result<tokio::runtime::Runtime> {
        let mut builder = match self.flavor {
            Flavor::SingleThreaded => tokio::runtime::Builder::new_current_thread(),
            Flavor::MultiThreaded {
                threads: ThreadsCount::Specific(count),
            } if count.get() == 1 => tokio::runtime::Builder::new_current_thread(),
            Flavor::MultiThreaded { threads } => {
                let mut builder = tokio::runtime::Builder::new_multi_thread();
                builder.worker_threads(match threads {
                    ThreadsCount::Specific(count) => count.get(),
                    ThreadsCount::ThreadPerCpu => {
                        std::thread::available_parallelism().map_or(1, NonZeroUsize::get)
                    }
                });
                builder
            }
        };
        builder.enable_all().build()
    }
}
