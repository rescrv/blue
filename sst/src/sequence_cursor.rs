#![allow(missing_docs)]

use keyvalint::{Cursor, KeyRef};

use super::{Error, TableMetadata};

////////////////////////////////////////// SequenceCursor //////////////////////////////////////////

// TODO(rescrv):  I don't like this cursor's structure.  Rethink it, then document.
pub struct SequenceCursor<T: TableMetadata, C: Cursor>
where
    C: TryFrom<T>,
    Error: From<<C as TryFrom<T>>::Error>,
{
    tables: Vec<T>,
    position: usize,
    cursor: C,
}

impl<T: TableMetadata + Clone, C: Cursor<Error = Error>> SequenceCursor<T, C>
where
    C: TryFrom<T>,
    Error: From<<C as TryFrom<T>>::Error>,
{
    pub fn new(tables: Vec<T>) -> Result<Self, Error> {
        assert!(!tables.is_empty());
        let position = 0;
        let mut cursor: C = C::try_from(tables[position].clone())?;
        cursor.seek_to_first()?;
        Ok(Self {
            tables,
            position,
            cursor,
        })
    }

    fn reposition(&mut self, idx: usize) -> Result<(), Error> {
        assert!(!self.tables.is_empty());
        if self.position != idx {
            self.position = idx;
            self.cursor = C::try_from(self.tables[self.position].clone())?;
        }
        Ok(())
    }
}

impl<T: TableMetadata + Clone, C: Cursor<Error = Error>> Cursor for SequenceCursor<T, C>
where
    C: TryFrom<T>,
    Error: From<<C as TryFrom<T>>::Error>,
{
    type Error = Error;

    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.reposition(0)?;
        self.cursor.seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.reposition(self.tables.len() - 1)?;
        self.cursor.seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        if self.tables.len() <= 1 {
            return self.cursor.seek(key);
        }
        let kref = KeyRef {
            key,
            timestamp: u64::max_value(),
        };
        let mut left = 0usize;
        let mut right = self.tables.len() - 1;

        while left < right {
            let mid = (left + right) / 2;
            if self.tables[mid].last_key() >= kref {
                right = mid;
            } else {
                left = mid + 1;
            }
        }

        self.reposition(left)?;
        self.cursor.seek(key)
    }

    fn prev(&mut self) -> Result<(), Error> {
        loop {
            self.cursor.prev()?;
            if self.cursor.value().is_none() && self.position > 0 {
                self.reposition(self.position - 1)?;
                self.cursor.seek_to_last()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), Error> {
        loop {
            self.cursor.next()?;
            if self.cursor.value().is_none() && self.position + 1 < self.tables.len() {
                self.reposition(self.position + 1)?;
                self.cursor.seek_to_first()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        self.cursor.key()
    }

    fn value(&self) -> Option<&[u8]> {
        self.cursor.value()
    }
}
