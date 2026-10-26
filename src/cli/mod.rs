use std::{
    any::{TypeId, type_name},
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::{Args, Parser};

use eyre::Context;
#[cfg(feature = "yaml")]
use figment::providers::YamlExtended;
use figment::{
    Figment,
    providers::{Env, Format, Json, Toml},
};
use hashbrown::hash_map;

use crate::{collections::HashMap, str::CompactString};

use crate::component_configs::{
    ComponentConfigs, ConfigHandle, DeserializeCfg, ErasedValue, IsComponentConfig,
};

use tokio::runtime;

pub mod rt;

#[derive(Debug, Clone, Copy, clap::Args)]
pub struct NoArgs {}

#[derive(Parser)]
struct AppArgs<A: Args> {
    #[clap(long, short)]
    /// Config file(s) for the application.
    config: Vec<PathBuf>,

    #[clap(flatten)]
    args: A,
}

struct ComponentConfigSetup {
    section: CompactString,
    deserialize: DeserializeCfg,
}

pub struct App {
    figment: Figment,
    env_prefix: Option<String>,
    component_configs: HashMap<TypeId, ComponentConfigSetup>,
}

impl App {
    /// Read configuration from `pfx`-prefixed environment variables.
    /// Nesting is expressed with `__`: `PFX_SYSTEM__RT__FLAVOR=...`.
    ///
    /// Environment variables take precedence over config files.
    pub fn env_prefix(mut self, pfx: &str) -> Self {
        self.env_prefix = Some(pfx.to_owned());

        self
    }

    pub fn optional<T: IsComponentConfig + Default>(mut self, section: &str) -> Self {
        let hash_map::Entry::Vacant(vacant) = self.component_configs.entry(TypeId::of::<T>())
        else {
            panic!("{} already exists", type_name::<T>())
        };

        vacant.insert(ComponentConfigSetup {
            section: section.into(),
            deserialize: |path, figment| {
                let res: figment::Result<T> = figment.extract_inner_lossy(path);
                let res = match res {
                    Ok(o) => Ok(o),
                    Err(e) => match &e.kind {
                        figment::error::Kind::MissingField(field) if field == path => {
                            Ok(T::default())
                        }
                        _ => Err(e).wrap_err("failed to deserialize section"),
                    },
                };

                res.map(|x| {
                    let x: ErasedValue = Arc::new(Mutex::new(x));
                    x
                })
            },
        });

        self
    }

    pub fn require<T: IsComponentConfig>(mut self, section: &str) -> Self {
        let hash_map::Entry::Vacant(vacant) = self.component_configs.entry(TypeId::of::<T>())
        else {
            panic!("{} already exists", type_name::<T>())
        };

        vacant.insert(ComponentConfigSetup {
            section: section.into(),
            deserialize: |path, figment| {
                let res: T = figment
                    .extract_inner_lossy(path)
                    .wrap_err("failed to deserialize")?;
                Ok(Arc::new(Mutex::new(res)))
            },
        });
        self
    }

    pub fn run<A>(
        self,
        f: impl Send + 'static + AsyncFnOnce(A, ComponentConfigs) -> eyre::Result<()>,
    ) -> eyre::Result<()>
    where
        A: Args,
    {
        const ONE: NonZeroUsize = NonZeroUsize::new(1).unwrap();

        let Self {
            mut figment,
            env_prefix,
            component_configs,
        } = self;
        let AppArgs {
            config: configs,
            args,
        } = AppArgs::<A>::parse();

        for config in configs {
            // TODO: support glob? Expand home?
            let component = config.components().next_back().ok_or_else(|| {
                eyre::eyre!("{} must contain at least one component", config.display())
            })?;
            let last = component
                .as_os_str()
                .as_encoded_bytes()
                .to_ascii_lowercase();

            if config.is_dir() {
                eyre::bail!(
                    "{} is a directory, directory configs are not supported yet",
                    config.display()
                );
            } else if last.ends_with(b".yaml") || last.ends_with(b".yml") {
                #[cfg(feature = "yaml")]
                {
                    figment = figment.merge(YamlExtended::file(config.as_path()));
                }
                #[cfg(not(feature = "yaml"))]
                eyre::bail!("{} is YAML: enable eva's `yaml` feature", config.display());
            } else if last.ends_with(b".json") {
                figment = figment.merge(Json::file(config.as_path()));
            } else if last.ends_with(b".toml") {
                figment = figment.merge(Toml::file(config.as_path()));
            } else {
                eyre::bail!(
                    "{} has an unrecognized config extension, expected yaml/yml/json/toml",
                    config.display()
                );
            }
        }

        // Merged last: environment beats config files.
        if let Some(pfx) = env_prefix {
            figment = figment.merge(Env::prefixed(&pfx).split("__"));
        }

        let mut map = HashMap::default();
        for (type_id, setup) in component_configs {
            let handle = ConfigHandle::initial(setup.section, setup.deserialize, &figment)?;
            map.insert(type_id, Arc::new(handle));
        }

        let component_configs = ComponentConfigs::new(map);

        // As early as possible: everything before this point reports only
        // through the returned `eyre::Report` on stderr.
        crate::logging::logforth::setup(component_configs.get())
            .wrap_err("failed to initialize logging")?;

        let rt_config = component_configs.get::<rt::RtConfig>();

        let rt = match rt_config.flavor {
            rt::Flavor::MultiThreaded {
                threads: rt::ThreadsCount::Specific(ONE),
            }
            | rt::Flavor::SingleThreaded => runtime::Builder::new_current_thread(),
            rt::Flavor::MultiThreaded { threads } => {
                let mut b = runtime::Builder::new_multi_thread();
                b.worker_threads(match threads {
                    rt::ThreadsCount::Specific(s) => s.get(),
                    rt::ThreadsCount::ThreadPerCpu => {
                        std::thread::available_parallelism().map_or(1, NonZeroUsize::get)
                    }
                });

                b
            }
        }
        .enable_all()
        .build()
        .wrap_err("failed to create tokio runtime")?;

        drop(rt_config);

        rt.block_on(f(args, component_configs))
    }
}

impl Default for App {
    fn default() -> Self {
        let this = Self {
            component_configs: HashMap::default(),
            env_prefix: None,
            figment: Figment::new(),
        };

        this.optional::<rt::RtConfig>("system.rt")
            .optional::<crate::logging::logforth::Config>("system.logging")
    }
}
