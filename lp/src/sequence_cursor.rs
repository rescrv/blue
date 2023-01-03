use zerror::ZError;

use super::{Cursor, Error, KeyRef, KeyValueRef, TableMetadata};

////////////////////////////////////////// SequenceCursor //////////////////////////////////////////

pub struct SequenceCursor<T: TableMetadata, C: Cursor>
where
    C: From<T>,
{
    tables: Vec<T>,
    position: usize,
    cursor: C,
}

impl<T: TableMetadata + Clone, C: Cursor> SequenceCursor<T, C>
where
    C: From<T>,
{
    pub fn new(tables: Vec<T>) -> Result<Self, ZError<Error>> {
        assert!(!tables.is_empty());
        let position = 0;
        let mut cursor: C = C::from(tables[position].clone());
        cursor.seek_to_first()?;
        Ok(Self {
            tables,
            position,
            cursor,
        })
    }

    fn reposition(&mut self, idx: usize) {
        assert!(!self.tables.is_empty());
        self.position = idx;
        self.cursor = C::from(self.tables[self.position].clone());
    }
}

impl<T: TableMetadata + Clone, C: Cursor> Cursor for SequenceCursor<T, C>
where
    C: From<T>,
{
    fn seek_to_first(&mut self) -> Result<(), ZError<Error>> {
        self.reposition(0);
        self.cursor.seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), ZError<Error>> {
        self.reposition(self.tables.len() - 1);
        self.cursor.seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), ZError<Error>> {
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

        self.reposition(left);
        self.cursor.seek(key)
    }

    fn prev(&mut self) -> Result<(), ZError<Error>> {
        loop {
            self.cursor.prev()?;
            if self.cursor.value().is_none() && self.position > 0 {
                self.reposition(self.position - 1);
                self.cursor.seek_to_last()?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), ZError<Error>> {
        loop {
            self.cursor.next()?;
            if self.cursor.value().is_none() && self.position + 1 < self.tables.len() {
                self.reposition(self.position + 1);
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

    fn value(&self) -> Option<KeyValueRef> {
        self.cursor.value()
    }
}
