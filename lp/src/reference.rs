use std::collections::btree_set::BTreeSet;
use std::ops::Bound;

use super::{Error, KeyValuePair, KeyOptionalValuePair, compare_bytes};
use super::LowLevelIterator as LowLevelIteratorTrait;
use super::LowLevelKeyValueStore as LowLevelKeyValueStoreTrait;
use super::LowLevelTransaction as LowLevelTransactionTrait;

//////////////////////////////////////// KeyValueStoreTrait2 ///////////////////////////////////////

trait LowLevelKeyValueStoreTrait2: LowLevelKeyValueStoreTrait {
    fn insert(&mut self, entry: &Entry);
}

/////////////////////////////////////////////// Entry //////////////////////////////////////////////

#[derive(Clone, Debug, Eq)]
pub struct Entry {
    pub key: Vec<u8>,
    pub timestamp: u64,
    pub value: Option<Vec<u8>>,
}

impl<'a> PartialEq for Entry {
    fn eq(&self, rhs: &Entry) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl<'a> Ord for Entry {
    fn cmp(&self, rhs: &Entry) -> std::cmp::Ordering {
        let key1 = &self.key;
        let key2 = &rhs.key;
        compare_bytes(key1, key2)
            .then(self.timestamp.cmp(&rhs.timestamp).reverse())
    }
}

impl<'a> PartialOrd for Entry {
    fn partial_cmp(&self, rhs: &Entry) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Entry {
    pub const BOTTOM: Entry = Entry {
        key: Vec::new(),
        timestamp: 0,
        value: None,
    };
}

///////////////////////////////////////// LowLevelIterator /////////////////////////////////////////

pub enum LowLevelIterator<'a> {
    First { kvs: &'a LowLevelKeyValueStore, now: u64 },
    Last { kvs: &'a LowLevelKeyValueStore, now: u64 },
    Position { kvs: &'a LowLevelKeyValueStore, now: u64, key: Vec<u8>, timestamp: u64 }
}

impl<'a> LowLevelIterator<'a> {
    fn kvs(&self) -> &'a LowLevelKeyValueStore {
        match self {
            LowLevelIterator::First { kvs, now: _ } => { kvs }
            LowLevelIterator::Last { kvs, now: _ } => { kvs }
            LowLevelIterator::Position { kvs, now: _, key: _, timestamp: _ } => { kvs }
        }
    }

    fn now(&self) -> u64 {
        match self {
            LowLevelIterator::First { kvs: _, now } => { *now }
            LowLevelIterator::Last { kvs: _, now } => { *now }
            LowLevelIterator::Position { kvs: _, now, key: _, timestamp: _ } => { *now }
        }
    }
}

impl<'a> LowLevelIteratorTrait for LowLevelIterator<'a> {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        let kvs = self.kvs();
        let now = self.now();
        *self = LowLevelIterator::First { kvs, now };
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        let kvs = self.kvs();
        let now = self.now();
        *self = LowLevelIterator::Last { kvs, now };
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        let kvs = self.kvs();
        let now = self.now();
        let target = Entry {
            key: key.to_vec(),
            timestamp: now,
            value: None,
        };
        let entry = match kvs.entries.range((Bound::Included(target), Bound::Unbounded)).next().map(|x| (*x).clone()) {
            Some(entry) => { entry },
            None => { return self.seek_to_last(); }
        };
        *self = LowLevelIterator::Position { kvs, now, key: entry.key, timestamp: entry.timestamp };
        Ok(())
    }

    fn prev(&mut self) -> Result<Option<KeyOptionalValuePair>, Error> {
        let kvs = self.kvs();
        let now = self.now();
        let start = match self {
            LowLevelIterator::First { kvs: _, now: _ } => { return Ok(None); },
            LowLevelIterator::Last { kvs: _, now: _ } => { Bound::Unbounded },
            LowLevelIterator::Position { kvs: _, now: _, key, timestamp } => {
                let target = Entry {
                    key: key.to_vec(),
                    timestamp: *timestamp,
                    value: None,
                };
                Bound::Excluded(target)
            },
        };
        let entry = kvs.entries.range((start, Bound::Included(Entry::BOTTOM))).next();
        *self = match entry {
            Some(e) => {
                LowLevelIterator::Position {
                    kvs,
                    now,
                    key: e.key.clone(),
                    timestamp: e.timestamp,
                }
            },
            None => { LowLevelIterator::First { kvs, now } },
        };
        self.same()
    }

    fn next(&mut self) -> Result<Option<KeyOptionalValuePair>, Error> {
        let kvs = self.kvs();
        let now = self.now();
        let start = match self {
            LowLevelIterator::First { kvs: _, now: _ } => { Bound::Excluded(Entry::BOTTOM) },
            LowLevelIterator::Last { kvs: _, now: _ } => { return Ok(None); },
            LowLevelIterator::Position { kvs: _, now: _, key, timestamp } => {
                let target = Entry {
                    key: key.to_vec(),
                    timestamp: *timestamp,
                    value: None,
                };
                Bound::Excluded(target)
            },
        };
        let entry = kvs.entries.range((start, Bound::Unbounded)).next();
        *self = match entry {
            Some(e) => {
                LowLevelIterator::Position {
                    kvs,
                    now,
                    key: e.key.clone(),
                    timestamp: e.timestamp,
                }
            },
            None => { LowLevelIterator::Last { kvs, now } },
        };
        self.same()
    }

    fn same(&mut self) -> Result<Option<KeyOptionalValuePair>, Error> {
        match self {
            LowLevelIterator::First { kvs: _, now: _ } => { return Ok(None); },
            LowLevelIterator::Last { kvs: _, now: _ } => { return Ok(None); },
            LowLevelIterator::Position { kvs, now: _, key, timestamp } => {
                let target = Entry {
                    key: key.to_vec(),
                    timestamp: *timestamp,
                    value: None,
                };
                let start = Bound::Included(target);
                let entry = kvs.entries.range((start, Bound::Unbounded)).next();
                match entry {
                    Some(e) => {
                        Ok(Some(KeyOptionalValuePair {
                            key: &e.key,
                            timestamp: e.timestamp,
                            value: match &e.value {
                                Some(x) => Some(&x),
                                None => None,
                            },
                        }))
                    },
                    None => { Ok(None) },
                }
            },
        }
    }
}

/////////////////////////////////////// LowLevelKeyValueStore //////////////////////////////////////

#[derive(Default)]
pub struct LowLevelKeyValueStore {
    entries: BTreeSet<Entry>,
    now: u64,
}

impl LowLevelKeyValueStore {
    pub fn set_now(&mut self, now: u64) {
        self.now = now;
    }
}

impl LowLevelKeyValueStoreTrait for LowLevelKeyValueStore {
    fn get_at_timestamp<'a>(&'a self, key: &[u8], timestamp: u64) -> Option<KeyOptionalValuePair<'a>> {
        let key = key.to_vec();
        let entry = Entry {
            key: key,
            timestamp: timestamp,
            value: None,
        };
        let entry = match self.entries.range((Bound::Included(entry), Bound::Unbounded)).next() {
            Some(e) => e,
            None => { return None; }
        };
        match entry.value {
            Some(ref v) => {
                let kvp = KeyOptionalValuePair {
                    key: &entry.key,
                    timestamp: entry.timestamp,
                    value: Some(v),
                };
                Some(kvp)
            },
            None => {
                None
            }
        }
    }

    fn iter<'a>(&'a self) -> Box<dyn LowLevelIteratorTrait + 'a> {
        Box::new(LowLevelIterator::First { kvs: self, now: self.now })
    }

    fn scan<'a>(&'a self, key: &[u8], timestamp: u64) -> Result<Box<dyn LowLevelIteratorTrait + 'a>, Error> {
        let mut iter = Box::new(LowLevelIterator::First { kvs: self, now: timestamp });
        iter.seek(key)?;
        Ok(iter)
    }

    fn transact<'a>(&'a mut self, timestamp: u64) -> Box<dyn LowLevelTransactionTrait + 'a> {
        let xact = LowLevelTransaction {
            kvs: self,
            entries: BTreeSet::default(),
            now: timestamp,
        };
        Box::new(xact)
    }
}

impl LowLevelKeyValueStoreTrait2 for LowLevelKeyValueStore {
    fn insert(&mut self, entry: &Entry) {
        self.entries.insert(entry.clone());
    }
}

//////////////////////////////////////////// Transaction ///////////////////////////////////////////

struct LowLevelTransaction<'a> {
    kvs: &'a mut dyn LowLevelKeyValueStoreTrait2,
    entries: BTreeSet<Entry>,
    now: u64,
}

impl<'b> LowLevelKeyValueStoreTrait for LowLevelTransaction<'b> {
    fn get_at_timestamp<'a>(&'a self, key: &[u8], timestamp: u64) -> Option<KeyOptionalValuePair<'a>> {
        self.kvs.get_at_timestamp(key, timestamp)
    }

    fn iter<'a>(&'a self) -> Box<dyn LowLevelIteratorTrait + 'a> {
        self.kvs.iter()
    }

    fn scan<'a>(&'a self, key: &[u8], timestamp: u64) -> Result<Box<dyn LowLevelIteratorTrait + 'a>, Error> {
        self.kvs.scan(key, timestamp)
    }

    fn transact<'a>(&'a mut self, timestamp: u64) -> Box<dyn LowLevelTransactionTrait + 'a> {
        Box::new(LowLevelTransaction {
            kvs: self,
            entries: BTreeSet::default(),
            now: timestamp,
        })
    }
}

impl<'a> LowLevelKeyValueStoreTrait2 for LowLevelTransaction<'a> {
    fn insert(&mut self, entry: &Entry) {
        self.entries.insert(entry.clone());
    }
}

impl<'a> LowLevelTransactionTrait for LowLevelTransaction<'a> {
    fn commit(self) {
        for entry in self.entries {
            self.kvs.insert(&entry);
        }
    }

    fn get<'b>(&'b mut self, key: &[u8]) -> Option<KeyOptionalValuePair<'b>> {
        self.kvs.get_at_timestamp(key, self.now)
    }

    fn put(&mut self, key: &[u8], value: &[u8]) {
        let key = key.to_vec();
        let value = value.to_vec();
        let entry = Entry {
            key: key,
            timestamp: self.now,
            value: Some(value),
        };
        self.entries.insert(entry);
    }

    fn del(&mut self, key: &[u8]) {
        let key = key.to_vec();
        let entry = Entry {
            key: key,
            timestamp: self.now,
            value: None,
        };
        self.entries.insert(entry);
    }
}
