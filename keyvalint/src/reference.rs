use std::cell::RefCell;
use std::ops::{Bound, RangeBounds};

use super::{
    compare_bytes, Cursor as CursorTrait, KeyRef, KeyValueLoad as KeyValueLoadTrait, KeyValuePair,
    KeyValueRef, KeyValueStore as KeyValueStoreTrait, WriteBatch as WriteBatchTrait,
};

//////////////////////////////////////////// WriteBatch ////////////////////////////////////////////

/// A reference [super::WriteBatch].
#[derive(Default)]
pub struct WriteBatch {
    entries: Vec<KeyValuePair>,
}

impl WriteBatchTrait for WriteBatch {
    fn put(&mut self, key: &[u8], value: &[u8]) {
        let timestamp = 1;
        let value = Some(value);
        self.entries.push(KeyValuePair::from(KeyValueRef {
            key,
            timestamp,
            value,
        }));
    }

    fn del(&mut self, key: &[u8]) {
        let timestamp = 1;
        self.entries
            .push(KeyValuePair::from(KeyRef { key, timestamp }));
    }
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

/// A reference [super::Cursor].
#[derive(Clone, Debug)]
pub struct Cursor {
    entries: Vec<KeyValuePair>,
    index: isize,
    returned: bool,
}

impl CursorTrait for Cursor {
    type Error = String;

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        self.index = -1;
        self.returned = true;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        self.index = self.entries.len() as isize;
        self.returned = true;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error> {
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

    fn prev(&mut self) -> Result<(), Self::Error> {
        self.index -= 1;
        if self.index < 0 {
            self.seek_to_first()
        } else {
            self.returned = true;
            Ok(())
        }
    }

    fn next(&mut self) -> Result<(), Self::Error> {
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

    fn value(&self) -> Option<&'_ [u8]> {
        if self.index < 0 || self.index as usize >= self.entries.len() {
            None
        } else {
            self.entries[self.index as usize].value.as_deref()
        }
    }
}

impl From<Vec<KeyValuePair>> for Cursor {
    fn from(entries: Vec<KeyValuePair>) -> Self {
        let mut c = Self {
            entries,
            index: -1,
            returned: false,
        };
        // SAFETY(rescrv): Unwrap is safe because I know this implementation will never fail.
        c.seek_to_first().unwrap();
        c
    }
}

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

/// A reference [super::KeyValueStore]
#[derive(Clone, Debug, Default)]
pub struct KeyValueStore {
    entries: RefCell<Vec<KeyValuePair>>,
}

impl KeyValueStore {
    /// Convert this reference KeyValueStore into a KeyValueLoad.
    pub fn into_key_value_load(self) -> KeyValueLoad {
        let mut entries = self.entries.into_inner();
        entries.sort();
        KeyValueLoad { entries }
    }
}

impl KeyValueStoreTrait for KeyValueStore {
    type Error = String;
    type WriteBatch<'a> = WriteBatch;

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::default();
        wb.put(key, value);
        self.write(wb)
    }

    fn del(&self, key: &[u8]) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::default();
        wb.del(key);
        self.write(wb)
    }

    fn write(&self, mut write_batch: Self::WriteBatch<'_>) -> Result<(), Self::Error> {
        self.entries.borrow_mut().append(&mut write_batch.entries);
        Ok(())
    }
}

/////////////////////////////////////////// KeyValueLoad ///////////////////////////////////////////

/// A reference [super::KeyValueLoad].
#[derive(Clone, Debug, Default)]
pub struct KeyValueLoad {
    entries: Vec<KeyValuePair>,
}

impl KeyValueLoad {}

impl KeyValueLoadTrait for KeyValueLoad {
    type Error = String;
    type RangeScan<'a> = Cursor;

    fn load(&self, key: &[u8], is_tombstone: &mut bool) -> Result<Option<Vec<u8>>, Self::Error> {
        let timestamp = u64::MAX;
        let target = KeyRef { key, timestamp };
        Ok(match self.entries.binary_search(&target.into()) {
            Ok(index) => {
                *is_tombstone = self.entries[index].value.is_none();
                self.entries[index].value.clone()
            }
            Err(index) => {
                if index < self.entries.len()
                    && compare_bytes(&self.entries[index].key, key).is_eq()
                {
                    *is_tombstone = self.entries[index].value.is_none();
                    self.entries[index].value.clone()
                } else {
                    None
                }
            }
        })
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        fn key_bound_to_key_ref_bound<U: AsRef<[u8]>>(
            bound: &Bound<U>,
            timestamp: u64,
        ) -> Bound<KeyRef<'_>> {
            match bound {
                Bound::Included(key) => Bound::Included(KeyRef {
                    key: key.as_ref(),
                    timestamp,
                }),
                Bound::Excluded(key) => Bound::Excluded(KeyRef {
                    key: key.as_ref(),
                    timestamp,
                }),
                Bound::Unbounded => Bound::Unbounded,
            }
        }
        let start_bound = key_bound_to_key_ref_bound(start_bound, u64::MAX);
        let end_bound = key_bound_to_key_ref_bound(end_bound, u64::MIN);
        let entries = self
            .entries
            .iter()
            .filter(|x| (start_bound..end_bound).contains(&KeyRef::from(*x)))
            .cloned()
            .collect::<Vec<_>>();
        Ok(Cursor::from(entries))
    }
}
