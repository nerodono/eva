use eva_core::{collections::HashMap, str::CompactString, trait_set};

mod builder;
mod secret;
pub use builder::Builder;
pub use secret::SecretString;

use std::{
    any::{Any, TypeId},
    fmt, mem,
    ops::Deref,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

pub(crate) type ErasedValue = Arc<Mutex<dyn Send + Sync + Any + 'static>>;
trait_set! {
    pub trait IsComponentConfig = Any + 'static + Clone + Send + Sync + serde::de::DeserializeOwned + serde::Serialize;
}

pub(crate) struct ConfigHandle {
    section: CompactString,
    recent: ErasedValue,
    generation: AtomicUsize,
}

impl ConfigHandle {
    pub(crate) fn new(section: CompactString, recent: ErasedValue) -> Self {
        Self {
            section,
            recent,
            generation: AtomicUsize::new(0),
        }
    }

    pub(crate) fn read<T: IsComponentConfig>(&self) -> Option<(T, usize)> {
        let cfg = self.recent.lock().unwrap();
        let generation = self.generation.load(Ordering::Relaxed);

        cfg.downcast_ref::<T>().map(|r| (r.clone(), generation))
    }
}

#[derive(Clone)]
pub struct ComponentConfigs {
    map: Arc<HashMap<TypeId, Arc<ConfigHandle>>>,
}

impl ComponentConfigs {
    pub(crate) fn new(map: HashMap<TypeId, Arc<ConfigHandle>>) -> Self {
        Self { map: Arc::new(map) }
    }

    #[track_caller]
    pub fn get<T: IsComponentConfig>(&self) -> ComponentConfig<T> {
        self.try_get::<T>().unwrap_or_else(|| {
            panic!(
                "configuration for {} does not exist",
                std::any::type_name::<T>()
            )
        })
    }

    pub fn try_get<T: IsComponentConfig>(&self) -> Option<ComponentConfig<T>> {
        let type_id = TypeId::of::<T>();
        self.map.get(&type_id).cloned().map(|handle| {
            let (cfg, generation) = handle.read::<T>().unwrap();
            ComponentConfig {
                cfg: Arc::new(cfg),
                generation,
                handle,
            }
        })
    }
}

#[derive(Clone)]
pub struct ComponentConfig<T> {
    cfg: Arc<T>,
    generation: usize,
    handle: Arc<ConfigHandle>,
}

impl<T: fmt::Debug> fmt::Debug for ComponentConfig<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <T as fmt::Debug>::fmt(&**self, f)
    }
}

impl<T: IsComponentConfig> ComponentConfig<T> {
    pub fn section(&self) -> &str {
        &self.handle.section
    }

    pub fn outdated(&self) -> bool {
        self.generation != self.handle.generation.load(Ordering::Relaxed)
    }

    /// Update component config. Returns previous config if something's changed.
    pub fn try_update(&mut self) -> Option<T> {
        let generation = self.handle.generation.load(Ordering::Relaxed);

        if generation == self.generation {
            None
        } else {
            Some(self.force_update())
        }
    }

    /// Update component config forcibly.
    pub fn force_update(&mut self) -> T {
        let (cfg, generation) = self.handle.read::<T>().unwrap();
        self.generation = generation;
        mem::replace(Arc::make_mut(&mut self.cfg), cfg)
    }
}

impl<T> Deref for ComponentConfig<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.cfg
    }
}
