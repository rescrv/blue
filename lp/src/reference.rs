use std::collections::btree_map::BTreeMap;
use std::ops::Bound;
use std::rc::Rc;

use super::{check_key_len, check_table_size, check_value_len, compare_key, Cursor, Error, KeyValuePair};

//////////////////////////////////////////////// Key ///////////////////////////////////////////////

#[derive(Clone, Debug, Eq)]
pub struct Key {
    pub key: Vec<u8>,
    pub timestamp: u64,
}

impl<'a> PartialEq for Key {
    fn eq(&self, rhs: &Key) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl<'a> Ord for Key {
    fn cmp(&self, rhs: &Key) -> std::cmp::Ordering {
        let key_lhs = &self.key;
        let key_rhs = &rhs.key;
        let ts_lhs = self.timestamp;
        let ts_rhs = rhs.timestamp;
        compare_key(key_lhs, ts_lhs, key_rhs, ts_rhs)
    }
}

impl<'a> PartialOrd for Key {
    fn partial_cmp(&self, rhs: &Key) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Key {
    pub const BOTTOM: Key = Key {
        key: Vec::new(),
        timestamp: 0,
    };
}

////////////////////////////////////////// ReferenceTable //////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct ReferenceTable {
    entries: Rc<BTreeMap<Key, Option<Vec<u8>>>>,
}

impl ReferenceTable {
    pub fn iterate(&self) -> ReferenceCursor {
        ReferenceCursor {
            entries: Rc::clone(&self.entries),
            position: TablePosition::default(),
        }
    }
}

///////////////////////////////////////// ReferenceBuilder /////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct ReferenceBuilder {
    entries: BTreeMap<Key, Option<Vec<u8>>>,
}

impl ReferenceBuilder {
    pub fn approximate_size(&self) -> usize {
        let mut size = 0;
        for (key, value) in self.entries.iter() {
            size += key.key.len()
                + 8
                + match value {
                    Some(v) => v.len(),
                    None => 0,
                };
        }
        size
    }

    pub fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        check_key_len(key)?;
        check_value_len(value)?;
        check_table_size(self.approximate_size())?;
        let key = Key {
            key: key.to_vec(),
            timestamp,
        };
        let value = value.to_vec();
        self.entries.insert(key, Some(value));
        Ok(())
    }

    pub fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        let key = Key {
            key: key.to_vec(),
            timestamp,
        };
        self.entries.insert(key, None);
        Ok(())
    }

    pub fn seal(self) -> Result<ReferenceTable, Error> {
        let entries = self.entries;
        Ok(ReferenceTable {
            entries: Rc::new(entries),
        })
    }
}

/////////////////////////////////////////// TablePosition //////////////////////////////////////////

#[derive(Clone, Debug)]
enum TablePosition {
    First,
    Last,
    Forward { last_key: Key },
    Reverse { last_key: Key },
}

impl Default for TablePosition {
    fn default() -> Self {
        TablePosition::First
    }
}

////////////////////////////////////////// ReferenceCursor /////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct ReferenceCursor {
    entries: Rc<BTreeMap<Key, Option<Vec<u8>>>>,
    position: TablePosition,
}

impl ReferenceCursor {
    pub fn get(&mut self, key: &[u8], timestamp: u64) -> Result<Option<KeyValuePair>, Error> {
        let start = Key {
            key: key.to_vec(),
            timestamp,
        };
        let limit = Key {
            key: key.to_vec(),
            timestamp: 0,
        };
        match self
            .entries
            .range((Bound::Included(start), Bound::Included(limit)))
            .next()
        {
            Some(entry) => Ok(Some(KeyValuePair {
                key: (&entry.0.key).into(),
                timestamp: entry.0.timestamp,
                value: match entry.1 {
                    Some(x) => Some(x.into()),
                    None => None,
                },
            })),
            None => Ok(None),
        }
    }
}

impl Cursor for ReferenceCursor {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.position = TablePosition::First;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.position = TablePosition::Last;
        Ok(())
    }

    fn seek(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        let target = Key {
            key: key.to_vec(),
            timestamp,
        };
        match self
            .entries
            .range((Bound::Included(target.clone()), Bound::Unbounded))
            .next()
        {
            Some(entry) => {
                let should_prev = entry.0 == &target;
                self.position = TablePosition::Forward { last_key: target };
                if should_prev {
                    self.prev()?;
                }
            }
            None => {
                self.position = TablePosition::Last;
            }
        };
        Ok(())
    }

    fn prev(&mut self) -> Result<Option<KeyValuePair>, Error> {
        let bound = match &self.position {
            TablePosition::First => {
                return Ok(None);
            }
            TablePosition::Last => Bound::Unbounded,
            TablePosition::Forward { last_key } => Bound::Excluded(last_key.clone()),
            TablePosition::Reverse { last_key } => Bound::Excluded(last_key.clone()),
        };
        match self
            .entries
            .range((Bound::Included(Key::BOTTOM), bound))
            .rev()
            .next()
        {
            Some(entry) => {
                self.position = TablePosition::Reverse {
                    last_key: entry.0.clone(),
                };
                Ok(Some(KeyValuePair {
                    key: (&entry.0.key).into(),
                    timestamp: entry.0.timestamp,
                    value: match entry.1 {
                        Some(v) => Some(v.into()),
                        None => None,
                    },
                }))
            }
            None => {
                self.position = TablePosition::First;
                Ok(None)
            }
        }
    }

    fn next(&mut self) -> Result<Option<KeyValuePair>, Error> {
        let bound = match &self.position {
            TablePosition::First => Bound::Excluded(Key::BOTTOM),
            TablePosition::Last => {
                return Ok(None);
            }
            TablePosition::Forward { last_key } => Bound::Excluded(last_key.clone()),
            TablePosition::Reverse { last_key } => Bound::Excluded(last_key.clone()),
        };
        match self.entries.range((bound, Bound::Unbounded)).next() {
            Some(entry) => {
                self.position = TablePosition::Forward {
                    last_key: entry.0.clone(),
                };
                Ok(Some(KeyValuePair {
                    key: (&entry.0.key).into(),
                    timestamp: entry.0.timestamp,
                    value: match entry.1 {
                        Some(v) => Some(v.into()),
                        None => None,
                    },
                }))
            }
            None => {
                self.position = TablePosition::Last;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod keys {
    use super::*;

    #[test]
    fn cmp() {
        let kvp1 = Key {
            key: "key1".into(),
            timestamp: 99,
        };
        let kvp2 = Key {
            key: "key1".into(),
            timestamp: 42,
        };
        let kvp3 = Key {
            key: "key2".into(),
            timestamp: 99,
        };
        assert!(kvp1 < kvp2);
        assert!(kvp2 < kvp3);
        assert!(kvp1 < kvp3);
    }
}

#[cfg(test)]
mod tables {
    use super::*;

    #[test]
    fn empty() {
        let table = ReferenceBuilder::default().seal().unwrap();
        let mut iter = table.iterate();
        let got = iter.next().unwrap();
        assert_eq!(None, got);
    }
}

#[cfg(test)]
mod guacamole {
    use super::*;

    #[test]
    fn human_guacamole_5() {
        let mut builder = ReferenceBuilder::default();
        builder
            .put("4".as_bytes(), 5220327133503220768, "".as_bytes())
            .unwrap();
        builder
            .put("A".as_bytes(), 2365635627947495809, "".as_bytes())
            .unwrap();
        builder
            .put("E".as_bytes(), 17563921251225492277, "".as_bytes())
            .unwrap();
        builder
            .put("I".as_bytes(), 3844377046565620216, "".as_bytes())
            .unwrap();
        builder
            .put("J".as_bytes(), 14848435744026832213, "".as_bytes())
            .unwrap();
        builder.del("U".as_bytes(), 8329339752768468916).unwrap();
        builder
            .put("g".as_bytes(), 10374159306796994843, "".as_bytes())
            .unwrap();
        builder
            .put("k".as_bytes(), 4092481979873166344, "".as_bytes())
            .unwrap();
        builder
            .put("t".as_bytes(), 7790837488841419319, "".as_bytes())
            .unwrap();
        builder
            .put("v".as_bytes(), 2133827469768204743, "".as_bytes())
            .unwrap();
        let block = builder.seal().unwrap();
        // Top of loop seeks to: "I"@13021764449837349261
        let mut cursor = block.iterate();
        cursor.seek("I".as_bytes(), 13021764449837349261).unwrap();
        let got = cursor.prev().unwrap();
        let exp = KeyValuePair {
            key: "E".into(),
            timestamp: 17563921251225492277,
            value: Some("".into()),
        };
        assert_eq!(Some(exp), got);
    }
}
