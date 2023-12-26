use std::ops::Bound;

use rocksdb::{DBIterator, Direction, IteratorMode, WriteBatch, WriteOptions, DB};

use crate::KeyRef;

impl super::WriteBatch for WriteBatch {
    fn put(&mut self, key: &[u8], _: u64, value: &[u8]) {
        self.put(key, value);
    }

    fn del(&mut self, key: &[u8], _: u64) {
        self.delete(key);
    }
}

impl<'a> super::Cursor for DBIterator<'a> {
    type Error = String;

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        todo!();
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        todo!();
    }

    fn seek(&mut self, _key: &[u8]) -> Result<(), Self::Error> {
        todo!();
    }

    fn prev(&mut self) -> Result<(), Self::Error> {
        todo!();
    }

    fn next(&mut self) -> Result<(), Self::Error> {
        if let Some(item) = <DBIterator<'a> as Iterator>::next(self) {
            match item {
                Ok(_) => {}
                Err(err) => {
                    return Err(format!("rocksdb iterator error: {}", err));
                }
            }
        }
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        todo!();
    }

    fn value(&self) -> Option<&'_ [u8]> {
        todo!();
    }
}

pub struct KeyValueStore {
    db: DB,
}

impl From<DB> for KeyValueStore {
    fn from(db: DB) -> Self {
        Self { db }
    }
}

impl super::KeyValueStore for KeyValueStore {
    type Error = String;
    type WriteBatch<'a> = WriteBatch;

    fn put(&self, key: &[u8], _: u64, value: &[u8]) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::default();
        wb.put(key, value);
        self.write(wb)
    }

    fn del(&self, key: &[u8], _: u64) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::default();
        wb.delete(key);
        self.write(wb)
    }

    fn write(&self, write_batch: Self::WriteBatch<'_>) -> Result<(), Self::Error> {
        let mut write_options = WriteOptions::default();
        write_options.set_sync(true);
        match self.db.write_opt(write_batch, &write_options) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("rocksdb error: {err:?}")),
        }
    }
}

impl super::KeyValueLoad for KeyValueStore {
    type Error = String;
    type RangeScan<'a> = DBIterator<'a>
        where
            Self: 'a;

    fn get<'a>(&self, key: &[u8], _: u64) -> Result<Option<Vec<u8>>, Self::Error> {
        self.db
            .get(key)
            .map_err(|err| format!("rocksdb get error: {}", err))
    }

    fn load(&self, _: &[u8], _: u64, _: &mut bool) -> Result<Option<Vec<u8>>, Self::Error> {
        Err("rocksdb KeyValueLoad interface doesn't support load".to_string())
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        _: u64,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        if let (Bound::Included(start_bound), Bound::Unbounded) = (start_bound, end_bound) {
            Ok(self
                .db
                .iterator(IteratorMode::From(start_bound.as_ref(), Direction::Forward)))
        } else {
            Err("Only a starting inclusive range bound is supported.".to_string())
        }
    }
}
