use std::ops::Bound;
use std::sync::atomic::{self, AtomicUsize};

use keyvalint::{compare_bytes, Cursor, Key, KeyRef};
use skipfree::{SkipList, SkipListIterator};
use sst::bounds_cursor::BoundsCursor;
use sst::pruning_cursor::PruningCursor;

use super::WriteBatch;
use crate::Error;

///////////////////////////////////////////// MemTable /////////////////////////////////////////////

#[derive(Default)]
pub struct MemTable {
    skiplist: SkipList<Key, Option<Vec<u8>>>,
    approximate_size: AtomicUsize,
}

impl MemTable {
    pub fn approximate_size(&self) -> usize {
        self.approximate_size.load(atomic::Ordering::Relaxed)
    }

    pub fn cursor(&self) -> SkipListIteratorWrapper {
        let iter = self.skiplist.iter();
        SkipListIteratorWrapper { iter }
    }

    pub fn write(&self, write_batch: &mut WriteBatch) -> Result<(), Error> {
        for entry in write_batch.entries.iter() {
            self.approximate_size.fetch_add(
                entry.key.len() + entry.value.as_ref().map(|x| x.len()).unwrap_or_default() + 16,
                atomic::Ordering::Relaxed,
            );
            let key = Key::from(entry);
            let value = entry.value.clone();
            self.skiplist.insert(key, value);
        }
        Ok(())
    }

    pub fn load(
        &self,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, sst::Error> {
        let mut cursor = self.skiplist.iter();
        // TODO(rescrv): Make it so I can use a KeyRef on the iterator.
        cursor.seek(&Key {
            key: key.to_vec(),
            timestamp,
        });
        if cursor.is_valid() && compare_bytes(cursor.key().key.as_slice(), key).is_eq() {
            *is_tombstone = cursor.value().is_none();
            Ok(cursor.value().clone())
        } else {
            Ok(None)
        }
    }

    pub fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<MemTableCursor, sst::Error> {
        let iter = self.skiplist.iter();
        let wrapper = SkipListIteratorWrapper { iter };
        let cursor = PruningCursor::new(wrapper, timestamp)?;
        let cursor = BoundsCursor::new(cursor, start_bound, end_bound)?;
        Ok(MemTableCursor { cursor })
    }
}

////////////////////////////////////// SkipListIteratorWrapper /////////////////////////////////////

pub struct SkipListIteratorWrapper {
    iter: SkipListIterator<Key, Option<Vec<u8>>>,
}

impl Cursor for SkipListIteratorWrapper {
    type Error = sst::Error;

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        self.iter.seek_to_first();
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        self.iter.seek_to_last();
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error> {
        self.iter.seek(&Key {
            key: key.into(),
            timestamp: u64::MAX,
        });
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Self::Error> {
        self.iter.prev();
        Ok(())
    }

    fn next(&mut self) -> Result<(), Self::Error> {
        self.iter.next();
        Ok(())
    }

    fn key(&self) -> Option<keyvalint::KeyRef<'_>> {
        if self.iter.is_valid() {
            Some(KeyRef::from(self.iter.key()))
        } else {
            None
        }
    }

    fn value(&self) -> Option<&[u8]> {
        if self.iter.is_valid() {
            self.iter.value().as_ref().map(|x| x.as_slice())
        } else {
            None
        }
    }
}

////////////////////////////////////////// MemTableCursor //////////////////////////////////////////

pub struct MemTableCursor {
    cursor: BoundsCursor<PruningCursor<SkipListIteratorWrapper, sst::Error>, sst::Error>,
}

impl Cursor for MemTableCursor {
    type Error = sst::Error;

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        self.cursor.seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        self.cursor.seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error> {
        self.cursor.seek(key)
    }

    fn prev(&mut self) -> Result<(), Self::Error> {
        self.cursor.prev()
    }

    fn next(&mut self) -> Result<(), Self::Error> {
        self.cursor.next()
    }

    fn key(&self) -> Option<keyvalint::KeyRef<'_>> {
        self.cursor.key()
    }

    fn value(&self) -> Option<&[u8]> {
        self.cursor.value()
    }
}
