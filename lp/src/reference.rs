use std::collections::btree_map::BTreeMap;
use std::ops::Bound;

use super::Table as TableTrait;
use super::TableBuilder as TableBuilderTrait;
use super::TableCursor as TableCursorTrait;
use super::{compare_bytes, Error, KeyValuePair};

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
        compare_bytes(key_lhs, key_rhs).then(ts_rhs.cmp(&ts_lhs))
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

/////////////////////////////////////////////// Table //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
struct Table {
    entries: BTreeMap<Key, Option<Vec<u8>>>,
}

impl<'a> TableTrait<'a> for Table {
    type Builder = TableBuilder;
    type Cursor = TableCursor<'a>;

    fn get(&'a self, key: &[u8], timestamp: u64) -> Option<KeyValuePair<'a>> {
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
            Some(entry) => Some(KeyValuePair {
                key: &entry.0.key,
                timestamp: entry.0.timestamp,
                value: match entry.1 {
                    Some(x) => Some(&x),
                    None => None,
                },
            }),
            None => None,
        }
    }

    fn iterate(&'a self) -> Self::Cursor {
        TableCursor {
            table: self,
            position: TablePosition::default(),
        }
    }
}

/////////////////////////////////////////// TableBuilder ///////////////////////////////////////////

#[derive(Clone, Debug, Default)]
struct TableBuilder {
    table: Table,
}

impl<'a> TableBuilderTrait<'a> for TableBuilder {
    type Table = Table;

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        let key = Key {
            key: key.to_vec(),
            timestamp,
        };
        let value = value.to_vec();
        self.table.entries.insert(key, Some(value));
        Ok(())
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        let key = Key {
            key: key.to_vec(),
            timestamp,
        };
        self.table.entries.insert(key, None);
        Ok(())
    }

    fn seal(self) -> Result<Table, Error> {
        Ok(self.table)
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

//////////////////////////////////////////// TableCursor ///////////////////////////////////////////

#[derive(Clone, Debug)]
struct TableCursor<'a> {
    table: &'a Table,
    position: TablePosition,
}

impl<'a> TableCursorTrait<'a> for TableCursor<'a> {
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
            .table
            .entries
            .range((Bound::Included(target), Bound::Unbounded))
            .next()
        {
            Some(entry) => {
                self.position = TablePosition::Forward {
                    last_key: entry.0.clone(),
                };
                self.prev()?;
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
            .table
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
                    key: &entry.0.key,
                    timestamp: entry.0.timestamp,
                    value: match entry.1 {
                        Some(v) => Some(&v),
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
        match self.table.entries.range((bound, Bound::Unbounded)).next() {
            Some(entry) => {
                self.position = TablePosition::Forward {
                    last_key: entry.0.clone(),
                };
                Ok(Some(KeyValuePair {
                    key: &entry.0.key,
                    timestamp: entry.0.timestamp,
                    value: match entry.1 {
                        Some(v) => Some(&v),
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
            key: "key1".as_bytes().to_vec(),
            timestamp: 99,
        };
        let kvp2 = Key {
            key: "key1".as_bytes().to_vec(),
            timestamp: 42,
        };
        let kvp3 = Key {
            key: "key2".as_bytes().to_vec(),
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
        let table = TableBuilder::default().seal().unwrap();
        let mut iter = table.iterate();
        let got = iter.next().unwrap();
        assert_eq!(None, got);
    }
}

#[cfg(test)]
mod alphabet {
    use super::*;

    fn alphabet() -> Table {
        let mut builder = TableBuilder::default();
        builder.put("A".as_bytes(), 0, "a".as_bytes()).unwrap();
        builder.put("B".as_bytes(), 0, "b".as_bytes()).unwrap();
        builder.put("C".as_bytes(), 0, "c".as_bytes()).unwrap();
        builder.put("D".as_bytes(), 0, "d".as_bytes()).unwrap();
        builder.put("E".as_bytes(), 0, "e".as_bytes()).unwrap();
        builder.put("F".as_bytes(), 0, "f".as_bytes()).unwrap();
        builder.put("G".as_bytes(), 0, "g".as_bytes()).unwrap();
        builder.put("H".as_bytes(), 0, "h".as_bytes()).unwrap();
        builder.put("I".as_bytes(), 0, "i".as_bytes()).unwrap();
        builder.put("J".as_bytes(), 0, "j".as_bytes()).unwrap();
        builder.put("K".as_bytes(), 0, "k".as_bytes()).unwrap();
        builder.put("L".as_bytes(), 0, "l".as_bytes()).unwrap();
        builder.put("M".as_bytes(), 0, "m".as_bytes()).unwrap();
        builder.put("N".as_bytes(), 0, "n".as_bytes()).unwrap();
        builder.put("O".as_bytes(), 0, "o".as_bytes()).unwrap();
        builder.put("P".as_bytes(), 0, "p".as_bytes()).unwrap();
        builder.put("Q".as_bytes(), 0, "q".as_bytes()).unwrap();
        builder.put("R".as_bytes(), 0, "r".as_bytes()).unwrap();
        builder.put("S".as_bytes(), 0, "s".as_bytes()).unwrap();
        builder.put("T".as_bytes(), 0, "t".as_bytes()).unwrap();
        builder.put("U".as_bytes(), 0, "u".as_bytes()).unwrap();
        builder.put("V".as_bytes(), 0, "v".as_bytes()).unwrap();
        builder.put("W".as_bytes(), 0, "w".as_bytes()).unwrap();
        builder.put("X".as_bytes(), 0, "x".as_bytes()).unwrap();
        builder.put("Y".as_bytes(), 0, "y".as_bytes()).unwrap();
        builder.put("Z".as_bytes(), 0, "z".as_bytes()).unwrap();
        builder.seal().unwrap()
    }

    #[test]
    fn step_the_alphabet_forward() {
        let table = alphabet();
        let mut iter = table.iterate();
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // C
        let exp = KeyValuePair {
            key: "C".as_bytes(),
            timestamp: 0,
            value: Some("c".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // D-W
        for _ in 0..20 {
            let _got = iter.next().unwrap().unwrap();
        }
        // X
        let exp = KeyValuePair {
            key: "X".as_bytes(),
            timestamp: 0,
            value: Some("x".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
    }

    #[test]
    fn step_the_alphabet_reverse() {
        let table = alphabet();
        let mut iter = table.iterate();
        iter.seek_to_last().unwrap();
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // X
        let exp = KeyValuePair {
            key: "X".as_bytes(),
            timestamp: 0,
            value: Some("x".as_bytes()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // W-D
        for _ in 0..20 {
            let _got = iter.prev().unwrap().unwrap();
        }
        // C
        let exp = KeyValuePair {
            key: "C".as_bytes(),
            timestamp: 0,
            value: Some("c".as_bytes()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // Last
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
    }

    #[test]
    fn seek_to_at() {
        let table = alphabet();
        let mut iter = table.iterate();
        iter.seek("@".as_bytes(), 0).unwrap();
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
    }

    #[test]
    fn seek_to_z() {
        let table = alphabet();
        let mut iter = table.iterate();
        iter.seek("Z".as_bytes(), 0).unwrap();
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
    }

    #[test]
    fn two_steps_forward_one_step_reverse() {
        let table = alphabet();
        let mut iter = table.iterate();
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // C
        let exp = KeyValuePair {
            key: "C".as_bytes(),
            timestamp: 0,
            value: Some("c".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // D-W
        for _ in 0..21 {
            iter.next().unwrap();
            iter.next().unwrap();
            iter.prev().unwrap();
        }
        // X
        let exp = KeyValuePair {
            key: "X".as_bytes(),
            timestamp: 0,
            value: Some("x".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // X
        let exp = KeyValuePair {
            key: "X".as_bytes(),
            timestamp: 0,
            value: Some("x".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
    }

    #[test]
    fn two_steps_reverse_one_step_forward() {
        let table = alphabet();
        let mut iter = table.iterate();
        iter.seek_to_last().unwrap();
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Z
        let exp = KeyValuePair {
            key: "Z".as_bytes(),
            timestamp: 0,
            value: Some("z".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // X
        let exp = KeyValuePair {
            key: "X".as_bytes(),
            timestamp: 0,
            value: Some("x".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".as_bytes(),
            timestamp: 0,
            value: Some("y".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // W-D
        for _ in 0..21 {
            iter.prev().unwrap();
            iter.prev().unwrap();
            iter.next().unwrap();
        }
        // C
        let exp = KeyValuePair {
            key: "C".as_bytes(),
            timestamp: 0,
            value: Some("c".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // C
        let exp = KeyValuePair {
            key: "C".as_bytes(),
            timestamp: 0,
            value: Some("c".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".as_bytes(),
            timestamp: 0,
            value: Some("b".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // First
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // First
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
        // First
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
        // A
        let exp = KeyValuePair {
            key: "A".as_bytes(),
            timestamp: 0,
            value: Some("a".as_bytes()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
    }
}
