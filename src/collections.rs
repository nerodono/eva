pub use hashbrown::{Equivalent, HashTable, hash_map, hash_table};

use std::hash::BuildHasherDefault;

pub type HashMap<K, V> = hashbrown::HashMap<K, V, BuildHasherDefault<crate::hash::U64Hasher>>;
pub type HashSet<V> = hashbrown::HashSet<V, BuildHasherDefault<crate::hash::U64Hasher>>;
