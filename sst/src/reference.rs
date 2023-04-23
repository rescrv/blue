use std::rc::Rc;

use super::{
    check_key_len, check_table_size, check_value_len, Cursor, Error, KeyRef, KeyValuePair,
    KeyValueRef, TableMetadata,
};

////////////////////////////////////////// ReferenceTable //////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct ReferenceTable {
    entries: Rc<Vec<KeyValuePair>>,
}

impl ReferenceTable {
    pub fn cursor(&self) -> ReferenceCursor {
        ReferenceCursor {
            entries: Rc::clone(&self.entries),
            index: -1,
            returned: true,
        }
    }
}

impl TableMetadata for ReferenceTable {
    fn first_key(&self) -> KeyRef {
        if self.entries.is_empty() {
            KeyRef {
                key: "".as_bytes(),
                timestamp: 0,
            }
        } else {
            (&self.entries[0]).into()
        }
    }

    fn last_key(&self) -> KeyRef {
        if self.entries.is_empty() {
            KeyRef {
                key: "".as_bytes(),
                timestamp: 0,
            }
        } else {
            (&self.entries[self.entries.len() - 1]).into()
        }
    }
}

///////////////////////////////////////// ReferenceBuilder /////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct ReferenceBuilder {
    entries: Vec<KeyValuePair>,
    approximate_size: usize,
}

impl ReferenceBuilder {
    pub fn approximate_size(&self) -> usize {
        self.approximate_size
    }

    pub fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        check_key_len(key)?;
        check_value_len(value)?;
        self.approximate_size += key.len() + 8 + value.len();
        check_table_size(self.approximate_size)?;
        let kvp = KeyValuePair {
            key: key.into(),
            timestamp,
            value: Some(value.into()),
        };
        self.entries.push(kvp);
        Ok(())
    }

    pub fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        self.approximate_size += key.len() + 8;
        check_table_size(self.approximate_size)?;
        let kvp = KeyValuePair {
            key: key.into(),
            timestamp,
            value: None,
        };
        self.entries.push(kvp);
        Ok(())
    }

    pub fn seal(self) -> Result<ReferenceTable, Error> {
        let mut entries = self.entries;
        entries.sort();
        Ok(ReferenceTable {
            entries: Rc::new(entries),
        })
    }
}

////////////////////////////////////////// ReferenceCursor /////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct ReferenceCursor {
    entries: Rc<Vec<KeyValuePair>>,
    index: isize,
    returned: bool,
}

impl Cursor for ReferenceCursor {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.index = -1;
        self.returned = true;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.index = self.entries.len() as isize;
        self.returned = true;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        let target = KeyValuePair {
            key: key.into(),
            timestamp: u64::max_value(),
            value: None,
        };
        self.index = match self.entries.binary_search(&target) {
            Ok(index) => index,
            Err(index) => index,
        } as isize;
        self.returned = false;
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        self.index = if self.returned {
            self.index - 1
        } else {
            self.index - 1
        };
        if self.index < 0 {
            self.seek_to_first()
        } else {
            self.returned = true;
            Ok(())
        }
    }

    fn next(&mut self) -> Result<(), Error> {
        self.index = if self.returned {
            self.index + 1
        } else {
            self.index
        };
        if self.index as usize >= self.entries.len() {
            self.seek_to_last()
        } else {
            self.returned = true;
            Ok(())
        }
    }

    fn key(&self) -> Option<KeyRef> {
        if self.index < 0 || self.index as usize >= self.entries.len() {
            None
        } else {
            let kvp = &self.entries[self.index as usize];
            Some(KeyRef::from(kvp))
        }
    }

    fn value(&self) -> Option<KeyValueRef> {
        if self.index < 0 || self.index as usize >= self.entries.len() {
            None
        } else {
            let kvp = &self.entries[self.index as usize];
            Some(KeyValueRef::from(kvp))
        }
    }
}

impl From<ReferenceTable> for ReferenceCursor {
    fn from(table: ReferenceTable) -> Self {
        table.cursor()
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tables {
    use super::*;

    #[test]
    fn empty() {
        let table = ReferenceBuilder::default().seal().unwrap();
        let mut cursor = table.cursor();
        cursor.next().unwrap();
        let got = cursor.value();
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
        let mut cursor = block.cursor();
        cursor.seek("I".as_bytes()).unwrap();
        cursor.prev().unwrap();
        let got = cursor.value();
        let exp = KeyValueRef {
            key: "E".as_bytes(),
            timestamp: 17563921251225492277,
            value: Some("".as_bytes()),
        };
        assert_eq!(Some(exp), got);
        // Top of loop seeks to: "I"@13021764449837349261
        let mut cursor = block.cursor();
        cursor.seek("I".as_bytes()).unwrap();
        cursor.next().unwrap();
        let got = cursor.value();
        let exp = KeyValueRef {
            key: "I".as_bytes(),
            timestamp: 3844377046565620216,
            value: Some("".as_bytes()),
        };
        assert_eq!(Some(exp), got);
        // Prev will move to E.
        cursor.prev().unwrap();
        let got = cursor.value();
        let exp = KeyValueRef {
            key: "E".as_bytes(),
            timestamp: 17563921251225492277,
            value: Some("".as_bytes()),
        };
        assert_eq!(Some(exp), got);
    }
}
