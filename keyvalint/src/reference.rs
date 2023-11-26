use std::cell::RefCell;
use std::cmp::Ordering;
use std::ops::{Bound, RangeBounds};

use super::{
    compare_bytes, Cursor as CursorTrait, KeyRef, KeyValueLoad as KeyValueLoadTrait, KeyValuePair,
    KeyValueRef, KeyValueStore as KeyValueStoreTrait, WriteBatch as WriteBatchTrait,
};

//////////////////////////////////////////// WriteBatch ////////////////////////////////////////////

#[derive(Default)]
pub struct WriteBatch {
    entries: Vec<KeyValuePair>,
}

impl WriteBatchTrait for WriteBatch {
    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        let value = Some(value);
        self.entries.push(KeyValuePair::from(KeyValueRef {
            key,
            timestamp,
            value,
        }));
    }

    fn del(&mut self, key: &[u8], timestamp: u64) {
        self.entries
            .push(KeyValuePair::from(KeyRef { key, timestamp }));
    }
}

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct KeyValueStore {
    entries: RefCell<Vec<KeyValuePair>>,
}

impl KeyValueStore {
    pub fn into_key_value_load(self) -> KeyValueLoad {
        let mut entries = self.entries.into_inner();
        entries.sort();
        KeyValueLoad { entries }
    }
}

impl KeyValueStoreTrait for KeyValueStore {
    type Error = String;
    type WriteBatch = WriteBatch;

    fn write(&self, mut write_batch: Self::WriteBatch) -> Result<(), Self::Error> {
        self.entries.borrow_mut().append(&mut write_batch.entries);
        Ok(())
    }
}

/////////////////////////////////////////// KeyValueLoad ///////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct KeyValueLoad {
    entries: Vec<KeyValuePair>,
}

impl KeyValueLoad {}

impl KeyValueLoadTrait for KeyValueLoad {
    type Error = String;
    type Cursor<'a> = Cursor;

    fn get(&self, key: &[u8], timestamp: u64) -> Result<Option<&'_ [u8]>, Self::Error> {
        let target = KeyRef { key, timestamp };
        match self.entries.binary_search(&target.into()) {
            Ok(index) => Ok(self.entries[index].value.as_deref()),
            Err(index) => {
                if compare_bytes(&self.entries[index].key, key) == Ordering::Equal {
                    Ok(self.entries[index].value.as_deref())
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn range_scan<R: RangeBounds<[u8]>>(
        &self,
        range: R,
        timestamp: u64,
    ) -> Result<Self::Cursor<'_>, Self::Error> {
        fn key_bound_to_key_ref_bound(bound: Bound<&[u8]>, timestamp: u64) -> Bound<KeyRef<'_>> {
            match bound {
                Bound::Included(key) => Bound::Included(KeyRef { key, timestamp }),
                Bound::Excluded(key) => Bound::Excluded(KeyRef { key, timestamp }),
                Bound::Unbounded => Bound::Unbounded,
            }
        }
        let start_bound = key_bound_to_key_ref_bound(range.start_bound(), timestamp);
        let end_bound = key_bound_to_key_ref_bound(range.start_bound(), timestamp);
        let entries = self
            .entries
            .iter()
            .filter(|x| (start_bound..end_bound).contains(&KeyRef::from(*x)))
            .cloned()
            .collect::<Vec<_>>();
        Ok(Cursor::from(entries))
    }
}

////////////////////////////////////////// ReferenceCursor /////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct Cursor {
    entries: Vec<KeyValuePair>,
    index: isize,
    returned: bool,
}

impl CursorTrait for Cursor {
    type Error = String;

    fn reset(&mut self) -> Result<(), Self::Error> {
        self.seek_to_first()
    }

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
