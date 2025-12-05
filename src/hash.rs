// per https://doc.rust-lang.org/std/hash/index.html

use std::{
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
};

use tracing::instrument;

#[instrument(level = "trace")]
pub fn get_hash<THash: Hash + fmt::Debug + ?Sized>(value: &THash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
