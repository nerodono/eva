use crate::hash::{self, IsHash, State};

pub const FULL_SIZE: usize = 32;
pub type Hash = hash::Hash<Blake3, FULL_SIZE>;

#[derive(Debug, Clone, Default)]
pub struct Hasher(blake3::Hasher);

impl Hasher {
    pub fn finish(&self) -> Hash {
        let h = self.0.finalize();
        Hash::new(*h.as_bytes())
    }
}

impl State for Hasher {
    fn update(&mut self, buf: &[u8]) {
        self.0.update(buf);
    }
}

#[crate::data(copy, ord, crate = crate)]
pub enum Blake3 {}

impl IsHash for Blake3 {
    const FULL_SIZE: usize = FULL_SIZE;
}

pub fn hash(bytes: &[u8]) -> Hash {
    let h = blake3::hash(bytes);
    Hash::new(*h.as_bytes())
}
