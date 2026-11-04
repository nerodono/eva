use std::{
    any::{TypeId, type_name},
    sync::{Arc, Mutex},
};

use eva_core::{
    collections::{HashMap, hash_map},
    str::CompactString,
};

use super::{ComponentConfigs, ConfigHandle, ErasedValue, IsComponentConfig};

type Load = Box<dyn FnOnce(&Sources) -> eyre::Result<ErasedValue> + Send>;

struct Registration {
    section: CompactString,
    load: Load,
}

/// Registers typed component configuration independently of CLI or an executor.
#[derive(Default)]
pub struct Builder {
    registrations: HashMap<TypeId, Registration>,
    sources: Sources,
}

impl Builder {
    /// A section whose absence uses `T::default()`; malformed sections still fail.
    #[track_caller]
    pub fn optional<T: IsComponentConfig + Default>(self, section: &str) -> Self {
        self.register::<T>(section, || Ok(T::default()))
    }

    /// A section that must be supplied by a configuration provider or [`Self::set`].
    #[track_caller]
    pub fn require<T: IsComponentConfig>(self, section: &str) -> Self {
        let missing = section.to_owned();
        self.register::<T>(section, move || {
            eyre::bail!("missing configuration section {missing:?}")
        })
    }

    #[track_caller]
    fn register<T: IsComponentConfig>(
        mut self,
        section: &str,
        fallback: impl FnOnce() -> eyre::Result<T> + Send + 'static,
    ) -> Self {
        let hash_map::Entry::Vacant(entry) = self.registrations.entry(TypeId::of::<T>()) else {
            panic!("{} already exists", type_name::<T>())
        };
        let name = section.to_owned();
        entry.insert(Registration {
            section: section.into(),
            load: Box::new(move |sources| sources.load(&name, fallback).map(erase)),
        });
        self
    }

    /// Supply a final typed value, replacing any registration for `T`.
    /// Providers do not override this explicit value.
    pub fn set<T: IsComponentConfig>(mut self, section: &str, value: T) -> Self {
        self.registrations.insert(
            TypeId::of::<T>(),
            Registration {
                section: section.into(),
                load: Box::new(move |_| Ok(erase(value))),
            },
        );
        self
    }

    /// Merge a Figment provider; later providers override earlier ones.
    #[cfg(feature = "figment")]
    pub fn merge(mut self, provider: impl figment::Provider) -> Self {
        self.sources.figment = self.sources.figment.merge(provider);
        self
    }

    /// Add a JSON, TOML, or (with `yaml`) YAML file.
    #[cfg(feature = "figment")]
    pub fn file(mut self, path: impl AsRef<std::path::Path>) -> eyre::Result<Self> {
        use figment::providers::{Format, Json, Toml};
        let path = path.as_ref();
        if path.is_dir() {
            eyre::bail!(
                "{} is a directory, directory configs are not supported yet",
                path.display()
            );
        }
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        self.sources.figment = match extension.as_str() {
            "json" => self.sources.figment.merge(Json::file(path)),
            "toml" => self.sources.figment.merge(Toml::file(path)),
            #[cfg(feature = "yaml")]
            "yaml" | "yml" => self
                .sources
                .figment
                .merge(figment::providers::YamlExtended::file(path)),
            #[cfg(not(feature = "yaml"))]
            "yaml" | "yml" => {
                eyre::bail!("{} is YAML: enable eva-rt's `yaml` feature", path.display())
            }
            _ => eyre::bail!(
                "{} has an unrecognized config extension, expected yaml/yml/json/toml",
                path.display()
            ),
        };
        Ok(self)
    }

    /// Environment is merged last, above every file/provider; `__` separates nested keys.
    #[cfg(feature = "figment")]
    pub fn env_prefix(mut self, prefix: &str) -> Self {
        self.sources.env_prefix = Some(prefix.to_owned());
        self
    }

    pub fn build(self) -> eyre::Result<ComponentConfigs> {
        let Self {
            registrations,
            sources,
        } = self;
        let sources = sources.finish();
        let mut configs = HashMap::default();
        for (id, registration) in registrations {
            let value = (registration.load)(&sources)?;
            configs.insert(id, Arc::new(ConfigHandle::new(registration.section, value)));
        }
        Ok(ComponentConfigs::new(configs))
    }
}

fn erase<T: IsComponentConfig>(value: T) -> ErasedValue {
    Arc::new(Mutex::new(value))
}

#[derive(Default)]
struct Sources {
    #[cfg(feature = "figment")]
    figment: figment::Figment,
    #[cfg(feature = "figment")]
    env_prefix: Option<String>,
}

impl Sources {
    fn finish(self) -> Self {
        #[cfg(feature = "figment")]
        {
            let mut this = self;
            if let Some(prefix) = &this.env_prefix {
                this.figment = this
                    .figment
                    .merge(figment::providers::Env::prefixed(prefix).split("__"));
            }
            this
        }
        #[cfg(not(feature = "figment"))]
        self
    }

    fn load<T: IsComponentConfig>(
        &self,
        section: &str,
        fallback: impl FnOnce() -> eyre::Result<T>,
    ) -> eyre::Result<T> {
        #[cfg(feature = "figment")]
        {
            use eyre::Context as _;
            match self.figment.extract_inner_lossy(section) {
                Ok(value) => Ok(value),
                Err(error) => match &error.kind {
                    figment::error::Kind::MissingField(field) if field == section => fallback(),
                    _ => Err(error)
                        .wrap_err_with(|| format!("failed to deserialize section {section:?}")),
                },
            }
        }
        #[cfg(not(feature = "figment"))]
        {
            let _ = section;
            fallback()
        }
    }
}
