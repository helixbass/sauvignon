// per https://doc.rust-lang.org/std/hash/index.html

use std::hash::{DefaultHasher, Hash, Hasher};

pub fn get_hash<THash: Hash + ?Sized>(value: &THash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
