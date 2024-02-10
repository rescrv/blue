//! A generic KEY VALue INTerface for abstracting away key-value stores.  Used for comparing
//! key-value stores in the keyvalint_bench crate.  Different key-value stores will have varying
//! levels of support.

use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Bound;
use std::sync::Arc;

/// A reference key-value store.
#[cfg(feature = "reference")]
pub mod reference;

/// A RocksDB-backed key-value store.
#[cfg(feature = "rocksdb")]
pub mod rocksdb;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

/// The maximum length of a key.
pub const MAX_KEY_LEN: usize = 1usize << 14; /* 16KiB */
/// The maximum length of a value.
pub const MAX_VALUE_LEN: usize = 1usize << 15; /* 32KiB */
/// The maximum size of a write batch, in bytes.
pub const MAX_BATCH_LEN: usize = (1usize << 20) - (1usize << 16); /* 1MiB - 64KiB */

/// The default key is the zero key.
pub const DEFAULT_KEY: &[u8] = &[];
/// The default timestamp is 0.
pub const DEFAULT_TIMESTAMP: u64 = 0;
/// The zero key.  This is the empty byte string.
pub const MIN_KEY: &[u8] = &[];
/// The maximum key.  This is eleven `0xff` bytes.
pub const MAX_KEY: &[u8] = &[0xffu8; 11];

/// The recommended size of a table.
///
/// This is an approximate size.  This constant isn't intended to be a maximum size, but rather a
/// size that, once exceeded, will cause the table to return a TableFull error.  The general
/// pattern is that the block will exceed this size by up to one key-value pair, so subtract some
/// slop.  64MiB is overkill, but will last for awhile.
pub const TABLE_FULL_SIZE: usize = (1usize << 30) - (1usize << 26); /* 1GiB - 64MiB */

//////////////////////////////////////////////// Key ///////////////////////////////////////////////

/// A memory-owning Key.
#[derive(Clone, Debug)]
pub struct Key {
    /// The key for this Key.
    pub key: Vec<u8>,
    /// The timestamp for this Key.
    pub timestamp: u64,
}

impl Default for Key {
    fn default() -> Self {
        Self {
            key: DEFAULT_KEY.into(),
            timestamp: DEFAULT_TIMESTAMP,
        }
    }
}

impl Eq for Key {}

impl PartialEq for Key {
    fn eq(&self, rhs: &Key) -> bool {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.eq(&rhs)
    }
}

impl Ord for Key {
    fn cmp(&self, rhs: &Key) -> std::cmp::Ordering {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.cmp(&rhs)
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, rhs: &Key) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl<'a> From<KeyRef<'a>> for Key {
    fn from(kr: KeyRef<'a>) -> Self {
        Self {
            key: kr.key.into(),
            timestamp: kr.timestamp,
        }
    }
}

impl<'a> From<KeyValueRef<'a>> for Key {
    fn from(kvr: KeyValueRef<'a>) -> Self {
        Self {
            key: kvr.key.into(),
            timestamp: kvr.timestamp,
        }
    }
}

impl From<KeyValuePair> for Key {
    fn from(kvr: KeyValuePair) -> Self {
        Self {
            key: kvr.key,
            timestamp: kvr.timestamp,
        }
    }
}

impl From<&KeyValuePair> for Key {
    fn from(kvr: &KeyValuePair) -> Self {
        Self {
            key: kvr.key.clone(),
            timestamp: kvr.timestamp,
        }
    }
}

////////////////////////////////////////////// KeyRef //////////////////////////////////////////////

/// A shallow, easy-to-copy reference to a key.
#[derive(Copy, Clone, Debug)]
pub struct KeyRef<'a> {
    /// The key of this KeyRef.
    pub key: &'a [u8],
    /// The timestamp of this KeyRef.
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

impl<'a> From<&'a Key> for KeyRef<'a> {
    fn from(k: &'a Key) -> Self {
        Self {
            key: &k.key,
            timestamp: k.timestamp,
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

/////////////////////////////////////// KeyValuePair ///////////////////////////////////////

/// A KeyValuePair is an owned version of a key-value pair.
#[derive(Clone, Debug)]
pub struct KeyValuePair {
    /// The key of this KeyValuePair.
    pub key: Vec<u8>,
    /// The timestamp of this KeyValuePair.
    pub timestamp: u64,
    /// The value of this KeyValuePair.  None indicates a tombstone.
    pub value: Option<Vec<u8>>,
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

//////////////////////////////////////////// KeyValueRef ///////////////////////////////////////////

/// A KeyValueRef is an easy-to-copy version of a key-value pair.
#[derive(Clone, Debug)]
pub struct KeyValueRef<'a> {
    /// The key of this KeyValueRef.
    pub key: &'a [u8],
    /// The timestamp of this KeyValueRef.
    pub timestamp: u64,
    /// The value of this KeyValueRef.  None indicates a tombstone.
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

//////////////////////////////////////////// WriteBatch ////////////////////////////////////////////

/// A write batch aggregates writes to be written together.
pub trait WriteBatch {
    /// Append the key-value pair to the write batch.
    fn put(&mut self, key: &[u8], value: &[u8]);
    /// Append a tombstone to the write batch.
    fn del(&mut self, key: &[u8]);
}

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

/// A write-oriented key-value store.  [KeyValueStore] is a pun on register store.
pub trait KeyValueStore {
    /// The type of error returned by this KeyValueStore.
    type Error: Debug;
    /// The type of write batch accepted by this KeyValueStore.
    type WriteBatch<'a>: WriteBatch;

    /// Put the specified key as a single, isolated write.
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), Self::Error>;
    /// Delete the specified key as a single, isolated write by writing a tombstone.
    fn del(&self, key: &[u8]) -> Result<(), Self::Error>;
    /// Write the batch to the key-value store.  Whether this is atomic depends upon the key-value
    /// store itself.
    fn write(&self, write_batch: Self::WriteBatch<'_>) -> Result<(), Self::Error>;
}

impl<K: KeyValueStore> KeyValueStore for Arc<K> {
    type Error = K::Error;
    type WriteBatch<'a> = K::WriteBatch<'a>;

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), Self::Error> {
        K::put(self, key, value)
    }

    fn del(&self, key: &[u8]) -> Result<(), Self::Error> {
        K::del(self, key)
    }

    fn write(&self, write_batch: Self::WriteBatch<'_>) -> Result<(), Self::Error> {
        K::write(self, write_batch)
    }
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

/// A Cursor allows for iterating through data.
pub trait Cursor {
    /// The type of error returned by this cursor.
    type Error: Debug;

    /// Seek past the first valid key-value pair to a beginning-of-stream sentinel.
    fn seek_to_first(&mut self) -> Result<(), Self::Error>;

    /// Seek past the last valid key-value pair to an end-of-stream sentinel.
    fn seek_to_last(&mut self) -> Result<(), Self::Error>;

    /// Seek to this key.  After a call to seek, the values of [key] and [value] should return the
    /// sought-to key or the key that's lexicographically next after key.
    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error>;

    /// Advance the cursor forward to the lexicographically-previous key.
    fn prev(&mut self) -> Result<(), Self::Error>;

    /// Advance the cursor forward to the lexicographically-next key.
    fn next(&mut self) -> Result<(), Self::Error>;

    /// The key where this cursor is positioned, or None if the cursor is positioned at the bounds.
    fn key(&self) -> Option<KeyRef>;

    /// The value where this cursor is positioned, or None if the cursor is positioned at a
    /// tombstone or the limits of the cursor.
    fn value(&self) -> Option<&'_ [u8]>;

    /// Return a KeyValueRef corresponding to the current position of the cursor.  By default this
    /// will stitch together the values of `key()` and `value()` to make a [KeyValueRef].
    fn key_value(&self) -> Option<KeyValueRef> {
        if let (Some(kr), value) = (self.key(), self.value()) {
            Some(KeyValueRef {
                key: kr.key,
                timestamp: kr.timestamp,
                value,
            })
        } else {
            None
        }
    }
}

impl Cursor for () {
    type Error = ();

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn seek(&mut self, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn next(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        None
    }

    fn value(&self) -> Option<&'_ [u8]> {
        None
    }
}

impl<E: Debug> Cursor for Box<dyn Cursor<Error = E>> {
    type Error = E;

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        self.as_mut().seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        self.as_mut().seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error> {
        self.as_mut().seek(key)
    }

    fn prev(&mut self) -> Result<(), Self::Error> {
        self.as_mut().prev()
    }

    fn next(&mut self) -> Result<(), Self::Error> {
        self.as_mut().next()
    }

    fn key(&self) -> Option<KeyRef> {
        self.as_ref().key()
    }

    fn value(&self) -> Option<&'_ [u8]> {
        self.as_ref().value()
    }
}

/////////////////////////////////////////// KeyValueLoad ///////////////////////////////////////////

/// A read-oriented key-value store.  [KeyValueLoad] is a pun on register load.
pub trait KeyValueLoad {
    /// The type of error returned by this KeyValueLoad.
    type Error: Debug;
    /// The type of cursor returned from [range_scan].
    type RangeScan<'a>: Cursor<Error=Self::Error>
    where
        Self: 'a;

    /// Get the value associated with the key.  By default this will call load and discard the
    /// `is_tombstone` parameter.  This should be sufficient for every implementation.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let mut is_tombstone = false;
        self.load(key, &mut is_tombstone)
    }

    /// Load the newest key.  Specifies `is_tombstone` when the None value returned is a tombstone.
    fn load(&self, key: &[u8], is_tombstone: &mut bool) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Perform a range scan between the specified bounds.
    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
    ) -> Result<Self::RangeScan<'_>, Self::Error>;
}

impl<K: KeyValueLoad> KeyValueLoad for Arc<K> {
    type Error = K::Error;
    type RangeScan<'a> = K::RangeScan<'a>
    where
        Self: 'a;

    fn load(&self, key: &[u8], is_tombstone: &mut bool) -> Result<Option<Vec<u8>>, Self::Error> {
        K::load(self, key, is_tombstone)
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        K::range_scan(self, start_bound, end_bound)
    }
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

/// Compare the bytes lexicographically.
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

/// Compare the keys lexicograhically.
pub fn compare_key(
    key_lhs: &[u8],
    timestamp_lhs: u64,
    key_rhs: &[u8],
    timestamp_rhs: u64,
) -> Ordering {
    compare_bytes(key_lhs, key_rhs).then(timestamp_lhs.cmp(&timestamp_rhs).reverse())
}
