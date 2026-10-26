use std::num::NonZeroUsize;

use crate::data;

#[data(crate = crate)]
#[derive(Default)]
#[serde(default)]
pub struct RtConfig {
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
