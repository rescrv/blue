use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Bound, RangeBounds};

#[cfg(feature = "reference")]
pub mod reference;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MAX_KEY_LEN: usize = 1usize << 14; /* 16KiB */
pub const MAX_VALUE_LEN: usize = 1usize << 15; /* 32KiB */
pub const MAX_BATCH_LEN: usize = 1usize << 16; /* 64KiB */

////////////////////////////////////////////// KeyRef //////////////////////////////////////////////

#[derive(Copy, Clone, Debug)]
pub struct KeyRef<'a> {
    pub key: &'a [u8],
    pub timestamp: u64,
}

impl<'a> Eq for KeyRef<'a> {}

impl<'a> PartialEq for KeyRef<'a> {
    fn eq(&self, rhs: &KeyRef) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl<'a> Ord for KeyRef<'a> {
    fn cmp(&self, rhs: &KeyRef) -> std::cmp::Ordering {
        compare_key(self.key, self.timestamp, rhs.key, rhs.timestamp)
    }
}

impl<'a> PartialOrd for KeyRef<'a> {
    fn partial_cmp(&self, rhs: &KeyRef) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl<'a> PartialEq<Bound<KeyRef<'a>>> for KeyRef<'a> {
    fn eq(&self, rhs: &Bound<KeyRef>) -> bool {
        match rhs {
            Bound::Included(rhs) => self.eq(rhs),
            Bound::Excluded(rhs) => self.eq(rhs),
            Bound::Unbounded => false,
        }
    }
}

impl<'a> PartialOrd<Bound<KeyRef<'a>>> for KeyRef<'a> {
    fn partial_cmp(&self, rhs: &Bound<KeyRef>) -> Option<std::cmp::Ordering> {
        match rhs {
            Bound::Included(rhs) => self.partial_cmp(rhs),
            Bound::Excluded(rhs) => self.partial_cmp(rhs),
            Bound::Unbounded => Some(Ordering::Less),
        }
    }
}

impl<'a> PartialEq<KeyRef<'a>> for Bound<KeyRef<'a>> {
    fn eq(&self, rhs: &KeyRef<'a>) -> bool {
        match self {
            Bound::Included(lhs) => lhs.eq(rhs),
            Bound::Excluded(lhs) => lhs.eq(rhs),
            Bound::Unbounded => false,
        }
    }
}

impl<'a> PartialOrd<KeyRef<'a>> for Bound<KeyRef<'a>> {
    fn partial_cmp(&self, rhs: &KeyRef<'a>) -> Option<std::cmp::Ordering> {
        match self {
            Bound::Included(lhs) => lhs.partial_cmp(rhs),
            Bound::Excluded(lhs) => lhs.partial_cmp(rhs),
            Bound::Unbounded => Some(Ordering::Less),
        }
    }
}

impl<'a, 'b: 'a> From<&'a KeyValueRef<'b>> for KeyRef<'a> {
    fn from(kvr: &'a KeyValueRef<'b>) -> KeyRef<'a> {
        Self {
            key: kvr.key,
            timestamp: kvr.timestamp,
        }
    }
}

impl<'a> From<&'a KeyValuePair> for KeyRef<'a> {
    fn from(kvp: &'a KeyValuePair) -> Self {
        Self {
            key: &kvp.key,
            timestamp: kvp.timestamp,
        }
    }
}

//////////////////////////////////////////// KeyValueRef ///////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct KeyValueRef<'a> {
    pub key: &'a [u8],
    pub timestamp: u64,
    pub value: Option<&'a [u8]>,
}

impl<'a> Display for KeyValueRef<'a> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let key = String::from_utf8(
            self.key
                .iter()
                .flat_map(|b| std::ascii::escape_default(*b))
                .collect::<Vec<u8>>(),
        )
        .unwrap();
        if let Some(value) = self.value {
            let value = String::from_utf8(
                value
                    .iter()
                    .flat_map(|b| std::ascii::escape_default(*b))
                    .collect::<Vec<u8>>(),
            )
            .unwrap();
            write!(fmt, "\"{}\" @ {} -> \"{}\"", key, self.timestamp, value)
        } else {
            write!(fmt, "\"{}\" @ {} -> <TOMBSTONE>", key, self.timestamp)
        }
    }
}

impl<'a> Eq for KeyValueRef<'a> {}

impl<'a> PartialEq for KeyValueRef<'a> {
    fn eq(&self, rhs: &KeyValueRef) -> bool {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.eq(&rhs)
    }
}

impl<'a> Ord for KeyValueRef<'a> {
    fn cmp(&self, rhs: &KeyValueRef) -> std::cmp::Ordering {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.cmp(&rhs)
    }
}

impl<'a> PartialOrd for KeyValueRef<'a> {
    fn partial_cmp(&self, rhs: &KeyValueRef) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl<'a> From<&'a KeyValuePair> for KeyValueRef<'a> {
    fn from(kvp: &'a KeyValuePair) -> Self {
        let value = match &kvp.value {
            Some(value) => {
                let value: &'a [u8] = value;
                Some(value)
            }
            None => None,
        };
        Self {
            key: &kvp.key,
            timestamp: kvp.timestamp,
            value,
        }
    }
}

/////////////////////////////////////// KeyValuePair ///////////////////////////////////////

#[derive(Clone, Debug)]
pub struct KeyValuePair {
    pub key: Vec<u8>,
    pub timestamp: u64,
    pub value: Option<Vec<u8>>,
}

impl KeyValuePair {
    pub fn from_key_value_ref(kvr: &KeyValueRef<'_>) -> Self {
        Self {
            key: kvr.key.into(),
            timestamp: kvr.timestamp,
            value: kvr.value.map(|v| v.into()),
        }
    }
}

impl Eq for KeyValuePair {}

impl PartialEq for KeyValuePair {
    fn eq(&self, rhs: &KeyValuePair) -> bool {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.eq(&rhs)
    }
}

impl Ord for KeyValuePair {
    fn cmp(&self, rhs: &KeyValuePair) -> std::cmp::Ordering {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.cmp(&rhs)
    }
}

impl PartialOrd for KeyValuePair {
    fn partial_cmp(&self, rhs: &KeyValuePair) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl<'a> From<KeyRef<'a>> for KeyValuePair {
    fn from(kvr: KeyRef<'a>) -> Self {
        Self {
            key: kvr.key.into(),
            timestamp: kvr.timestamp,
            value: None,
        }
    }
}

impl<'a> From<KeyValueRef<'a>> for KeyValuePair {
    fn from(kvr: KeyValueRef<'a>) -> Self {
        Self {
            key: kvr.key.into(),
            timestamp: kvr.timestamp,
            value: kvr.value.map(|v| v.into()),
        }
    }
}

//////////////////////////////////////////// WriteBatch ////////////////////////////////////////////

pub trait WriteBatch: Default {
    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]);
    fn del(&mut self, key: &[u8], timestamp: u64);
}

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

pub trait KeyValueStore {
    type Error: Debug;
    type WriteBatch: WriteBatch;

    fn put(&self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::default();
        wb.put(key, timestamp, value);
        self.write(wb)
    }

    fn del(&self, key: &[u8], timestamp: u64) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::default();
        wb.del(key, timestamp);
        self.write(wb)
    }

    fn write(&self, write_batch: Self::WriteBatch) -> Result<(), Self::Error>;
}

/////////////////////////////////////////// KeyValueLoad ///////////////////////////////////////////

pub trait KeyValueLoad {
    type Error: Debug;
    type Cursor<'a>: Cursor
    where
        Self: 'a;

    fn get(&self, key: &[u8], timestamp: u64) -> Result<Option<&'_ [u8]>, Self::Error>;
    fn range_scan<R: RangeBounds<[u8]>>(&self, range: R, timestamp: u64) -> Result<Self::Cursor<'_>, Self::Error>;
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

pub trait Cursor {
    type Error;

    fn reset(&mut self) -> Result<(), Self::Error>;

    fn seek_to_first(&mut self) -> Result<(), Self::Error>;
    fn seek_to_last(&mut self) -> Result<(), Self::Error>;
    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error>;

    fn prev(&mut self) -> Result<(), Self::Error>;
    fn next(&mut self) -> Result<(), Self::Error>;

    fn key(&self) -> Option<KeyRef>;
    fn value(&self) -> Option<&'_ [u8]>;
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

// Content under CC By-Sa.  I just use as is, as can you.
// https://codereview.stackexchange.com/questions/233872/writing-slice-compare-in-a-more-compact-way
pub fn compare_bytes(a: &[u8], b: &[u8]) -> Ordering {
    for (ai, bi) in a.iter().zip(b.iter()) {
        match ai.cmp(bi) {
            Ordering::Equal => continue,
            ord => return ord,
        }
    }

    /* if every single element was equal, compare length */
    a.len().cmp(&b.len())
}
// End borrowed code

//////////////////////////////////////////// compare_key ///////////////////////////////////////////

pub fn compare_key(
    key_lhs: &[u8],
    timestamp_lhs: u64,
    key_rhs: &[u8],
    timestamp_rhs: u64,
) -> Ordering {
    compare_bytes(key_lhs, key_rhs).then(timestamp_lhs.cmp(&timestamp_rhs).reverse())
}
