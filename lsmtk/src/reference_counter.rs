use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct ReferenceCounter<T: Eq + Hash> {
    counts: Mutex<HashMap<T, u64>>,
}

impl<T: Eq + Hash + Debug> ReferenceCounter<T> {
    pub fn inc(&self, t: T) {
        let mut counts = self.counts.lock().unwrap();
        *counts.entry(t).or_insert(0) += 1;
    }

    pub fn dec(&self, t: T) -> bool {
        let mut counts = self.counts.lock().unwrap();
        match counts.entry(t) {
            Entry::Occupied(mut entry) => {
                if *entry.get() <= 1 {
                    entry.remove();
                    true
                } else {
                    *entry.get_mut() -= 1;
                    false
                }
            }
            Entry::Vacant(_) => false,
        }
    }
}
