use std::collections::btree_set::BTreeSet;
use std::ops::Bound;

use super::{Error, KeyValuePair, compare_bytes};
use super::Iterator as IteratorTrait;
use super::KeyValueStore as KeyValueStoreTrait;
use super::Transaction as TransactionTrait;

//////////////////////////////////////// KeyValueStoreTrait2 ///////////////////////////////////////

trait KeyValueStoreTrait2: KeyValueStoreTrait {
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

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

#[derive(Default)]
pub struct KeyValueStore {
    entries: BTreeSet<Entry>,
    now: u64,
}

impl KeyValueStore {
    pub fn set_now(&mut self, now: u64) {
        self.now = now;
    }
}

impl KeyValueStoreTrait for KeyValueStore {
    fn get_at_timestamp<'a>(&'a self, key: &[u8], timestamp: u64) -> Option<KeyValuePair<'a>> {
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
                let kvp = KeyValuePair {
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

    fn iter<'a>(&'a self) -> Box<dyn IteratorTrait + 'a> {
        Box::new(Iterator::First { kvs: self, now: self.now })
    }

    fn scan<'a>(&'a self, key: &[u8], timestamp: u64) -> Result<Box<dyn IteratorTrait + 'a>, Error> {
        let mut iter = Box::new(Iterator::First { kvs: self, now: timestamp });
        iter.seek(key)?;
        Ok(iter)
    }

    fn transact<'a>(&'a mut self, timestamp: u64) -> Box<dyn TransactionTrait + 'a> {
        let xact = Transaction {
            kvs: self,
            entries: BTreeSet::default(),
            now: timestamp,
        };
        Box::new(xact)
    }
}

impl KeyValueStoreTrait2 for KeyValueStore {
    fn insert(&mut self, entry: &Entry) {
        self.entries.insert(entry.clone());
    }
}

///////////////////////////////////////////// Iterator /////////////////////////////////////////////

pub enum Iterator<'a> {
    First { kvs: &'a KeyValueStore, now: u64 },
    Last { kvs: &'a KeyValueStore, now: u64 },
    Position { kvs: &'a KeyValueStore, now: u64, key: Vec<u8>, timestamp: u64 }
}

impl<'a> Iterator<'a> {
    fn kvs(&self) -> &'a KeyValueStore {
        match self {
            Iterator::First { kvs, now: _ } => { kvs }
            Iterator::Last { kvs, now: _ } => { kvs }
            Iterator::Position { kvs, now: _, key: _, timestamp: _ } => { kvs }
        }
    }

    fn now(&self) -> u64 {
        match self {
            Iterator::First { kvs: _, now } => { *now }
            Iterator::Last { kvs: _, now } => { *now }
            Iterator::Position { kvs: _, now, key: _, timestamp: _ } => { *now }
        }
    }
}

impl<'a> IteratorTrait for Iterator<'a> {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        let kvs = self.kvs();
        let now = self.now();
        *self = Iterator::First { kvs, now };
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        let kvs = self.kvs();
        let now = self.now();
        *self = Iterator::Last { kvs, now };
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
        *self = Iterator::Position { kvs, now, key: entry.key, timestamp: entry.timestamp };
        Ok(())
    }

    fn prev(&mut self) -> Result<Option<KeyValuePair>, Error> {
        let kvs = self.kvs();
        let now = self.now();
        let start = match self {
            Iterator::First { kvs: _, now: _ } => { return Ok(None); },
            Iterator::Last { kvs: _, now: _ } => { Bound::Unbounded },
            Iterator::Position { kvs: _, now: _, key, timestamp } => {
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
                Iterator::Position {
                    kvs,
                    now,
                    key: e.key.clone(),
                    timestamp: e.timestamp,
                }
            },
            None => { Iterator::First { kvs, now } },
        };
        self.same()
    }

    fn next(&mut self) -> Result<Option<KeyValuePair>, Error> {
        let kvs = self.kvs();
        let now = self.now();
        let start = match self {
            Iterator::First { kvs: _, now: _ } => { Bound::Excluded(Entry::BOTTOM) },
            Iterator::Last { kvs: _, now: _ } => { return Ok(None); },
            Iterator::Position { kvs: _, now: _, key, timestamp } => {
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
                Iterator::Position {
                    kvs,
                    now,
                    key: e.key.clone(),
                    timestamp: e.timestamp,
                }
            },
            None => { Iterator::Last { kvs, now } },
        };
        self.same()
    }

    fn same(&mut self) -> Result<Option<KeyValuePair>, Error> {
        match self {
            Iterator::First { kvs: _, now: _ } => { return Ok(None); },
            Iterator::Last { kvs: _, now: _ } => { return Ok(None); },
            Iterator::Position { kvs, now: _, key, timestamp } => {
                let target = Entry {
                    key: key.to_vec(),
                    timestamp: *timestamp,
                    value: None,
                };
                let start = Bound::Included(target);
                let entry = kvs.entries.range((start, Bound::Unbounded)).next();
                match entry {
                    Some(e) => {
                        Ok(Some(KeyValuePair {
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

//////////////////////////////////////////// Transaction ///////////////////////////////////////////

struct Transaction<'a> {
    kvs: &'a mut dyn KeyValueStoreTrait2,
    entries: BTreeSet<Entry>,
    now: u64,
}

impl<'b> KeyValueStoreTrait for Transaction<'b> {
    fn get_at_timestamp<'a>(&'a self, key: &[u8], timestamp: u64) -> Option<KeyValuePair<'a>> {
        self.kvs.get_at_timestamp(key, timestamp)
    }

    fn iter<'a>(&'a self) -> Box<dyn IteratorTrait + 'a> {
        self.kvs.iter()
    }

    fn scan<'a>(&'a self, key: &[u8], timestamp: u64) -> Result<Box<dyn IteratorTrait + 'a>, Error> {
        self.kvs.scan(key, timestamp)
    }

    fn transact<'a>(&'a mut self, timestamp: u64) -> Box<dyn TransactionTrait + 'a> {
        Box::new(Transaction {
            kvs: self,
            entries: BTreeSet::default(),
            now: timestamp,
        })
    }
}

impl<'a> KeyValueStoreTrait2 for Transaction<'a> {
    fn insert(&mut self, entry: &Entry) {
        self.entries.insert(entry.clone());
    }
}

impl<'a> TransactionTrait for Transaction<'a> {
    fn commit(self) {
        for entry in self.entries {
            self.kvs.insert(&entry);
        }
    }

    fn get<'b>(&'b mut self, key: &[u8]) -> Option<KeyValuePair<'b>> {
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
