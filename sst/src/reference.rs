//! Reference types for comparing sst and block behavior.

use std::rc::Rc;

use super::{
    check_key_len, check_table_size, check_value_len, Cursor, Error, KeyRef, KeyValuePair,
};

////////////////////////////////////////// ReferenceTable //////////////////////////////////////////

/// A ReferenceTable provides the table interface as a comparison for specifying behavior.
#[derive(Clone, Debug, Default)]
pub struct ReferenceTable {
    entries: Rc<Vec<KeyValuePair>>,
}

impl ReferenceTable {
    /// Return a new cursor over this table.
    pub fn cursor(&self) -> ReferenceCursor {
        ReferenceCursor {
            entries: Rc::clone(&self.entries),
            index: -1,
        }
    }
}

///////////////////////////////////////// ReferenceBuilder /////////////////////////////////////////

/// A builder that returns a ReferenceTable.
#[derive(Clone, Debug, Default)]
pub struct ReferenceBuilder {
    entries: Vec<KeyValuePair>,
    approximate_size: usize,
}

impl ReferenceBuilder {
    /// The approximate size of the builder.
    pub fn approximate_size(&self) -> usize {
        self.approximate_size
    }

    /// Put a key in the reference builder.
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

    /// Delete a key from the reference builder.
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

    /// Seal the reference builder and get a ReferenceTable.
    pub fn seal(self) -> Result<ReferenceTable, Error> {
        let mut entries = self.entries;
        entries.sort();
        Ok(ReferenceTable {
            entries: Rc::new(entries),
        })
    }
}

////////////////////////////////////////// ReferenceCursor /////////////////////////////////////////

/// A cursor over a reference table.
#[derive(Clone, Debug)]
pub struct ReferenceCursor {
    entries: Rc<Vec<KeyValuePair>>,
    index: isize,
}

impl Cursor for ReferenceCursor {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.index = -1;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.index = self.entries.len() as isize;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        let target = KeyValuePair {
            key: key.into(),
            timestamp: u64::MAX,
            value: None,
        };
        self.index = match self.entries.binary_search(&target) {
            Ok(index) => index,
            Err(index) => index,
        } as isize;
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        self.index -= 1;
        if self.index < 0 {
            self.seek_to_first()
        } else {
            Ok(())
        }
    }

    fn next(&mut self) -> Result<(), Error> {
        self.index += 1;
        if self.index as usize >= self.entries.len() {
            self.seek_to_last()
        } else {
            Ok(())
        }
    }

    fn key(&self) -> Option<KeyRef<'_>> {
        if self.index < 0 || self.index as usize >= self.entries.len() {
            None
        } else {
            let kvp = &self.entries[self.index as usize];
            Some(KeyRef::from(kvp))
        }
    }

    fn value(&self) -> Option<&[u8]> {
        if self.index < 0 || self.index as usize >= self.entries.len() {
            None
        } else {
            let kvp = &self.entries[self.index as usize];
            kvp.value.as_deref()
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
        let cursor = table.cursor();
        let got = cursor.key_value();
        assert_eq!(None, got);
    }
}
