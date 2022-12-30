extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use std::cmp;
use std::cmp::Ordering;

use buffertk::Buffer;

pub mod block;
pub mod file_manager;
pub mod reference;
pub mod sst;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MAX_KEY_LEN: usize = 1usize << 16; /* 64KiB */
pub const MAX_VALUE_LEN: usize = 1usize << 24; /* 16MiB */

// NOTE(rescrv):  This is an approximate size.  This constant isn't intended to be a maximum size,
// but rather a size that, once exceeded, will cause the table to return a TableFull error.  The
// general pattern is that the block will exceed this size by up to one key-value pair.
pub const TABLE_FULL_SIZE: usize = 1usize << 30; /* 1GiB */

fn check_key_len(key: &[u8]) -> Result<(), Error> {
    if key.len() > MAX_KEY_LEN {
        Err(Error::KeyTooLarge {
            length: key.len(),
            limit: MAX_KEY_LEN,
        })
    } else {
        Ok(())
    }
}

fn check_value_len(value: &[u8]) -> Result<(), Error> {
    if value.len() > MAX_VALUE_LEN {
        Err(Error::ValueTooLarge {
            length: value.len(),
            limit: MAX_VALUE_LEN,
        })
    } else {
        Ok(())
    }
}

fn check_table_size(size: usize) -> Result<(), Error> {
    if size >= TABLE_FULL_SIZE {
        Err(Error::TableFull {
            size,
            limit: TABLE_FULL_SIZE,
        })
    } else {
        Ok(())
    }
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    KeyTooLarge {
        length: usize,
        limit: usize,
    },
    ValueTooLarge {
        length: usize,
        limit: usize,
    },
    SortOrder {
        last_key: Vec<u8>,
        last_timestamp: u64,
        new_key: Vec<u8>,
        new_timestamp: u64,
    },
    TableFull {
        size: usize,
        limit: usize,
    },
    BlockTooSmall {
        length: usize,
        required: usize,
    },
    UnpackError {
        error: prototk::Error,
        context: String,
    },
    Corruption {
        context: String,
    },
    LogicError {
        context: String,
    },
    IoError {
        what: std::io::Error,
    },
    TooManyOpenFiles {
        limit: usize,
    },
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::IoError { what }
    }
}

////////////////////////////////////////////// KeyRef //////////////////////////////////////////////

#[derive(Clone, Debug)]
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

impl<'a> From<&'a KeyValuePair> for KeyRef<'a> {
    fn from(kvp: &'a KeyValuePair) -> KeyRef<'a> {
        Self {
            key: kvp.key.as_bytes(),
            timestamp: kvp.timestamp,
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

//////////////////////////////////////////// KeyValueRef ///////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct KeyValueRef<'a> {
    pub key: &'a [u8],
    pub timestamp: u64,
    pub value: Option<&'a [u8]>,
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
    fn from(kvp: &'a KeyValuePair) -> KeyValueRef<'a> {
        Self {
            key: kvp.key.as_bytes(),
            timestamp: kvp.timestamp,
            value: match &kvp.value {
                Some(v) => Some(v.as_bytes()),
                None => None,
            }
        }
    }
}

/////////////////////////////////////// KeyValuePair ///////////////////////////////////////

#[derive(Clone, Debug)]
pub struct KeyValuePair {
    pub key: Buffer,
    pub timestamp: u64,
    pub value: Option<Buffer>,
}

impl KeyValuePair {
    pub fn from_key_value_ref<'a>(kvr: &KeyValueRef<'a>) -> Self {
        Self {
            key: kvr.key.into(),
            timestamp: kvr.timestamp,
            value: match kvr.value {
                Some(x) => Some(x.into()),
                None => None,
            },
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

impl<'a> From<KeyValueRef<'a>> for KeyValuePair {
    fn from(kvr: KeyValueRef<'a>) -> Self {
        Self {
            key: kvr.key.into(),
            timestamp: kvr.timestamp,
            value: match kvr.value {
                Some(v) => Some(v.into()),
                None => None,
            },
        }
    }
}

////////////////////////////////////////////// Builder /////////////////////////////////////////////

pub trait Builder {
    type Sealed;

    fn approximate_size(&self) -> usize;

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error>;
    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error>;

    fn seal(self) -> Result<Self::Sealed, Error>;
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

pub trait Cursor {
    fn seek_to_first(&mut self) -> Result<(), Error>;
    fn seek_to_last(&mut self) -> Result<(), Error>;
    fn seek(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error>;

    fn prev(&mut self) -> Result<(), Error>;
    fn next(&mut self) -> Result<(), Error>;
    fn value(&self) -> Option<KeyValueRef>;
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

// Content under CC By-Sa.  I just use as is, as can you.
// https://codereview.stackexchange.com/questions/233872/writing-slice-compare-in-a-more-compact-way
pub fn compare_bytes(a: &[u8], b: &[u8]) -> cmp::Ordering {
    for (ai, bi) in a.iter().zip(b.iter()) {
        match ai.cmp(&bi) {
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

//////////////////////////////////////////// divide_keys ///////////////////////////////////////////

/// Return a key that is >= lhs and < rhs.
fn divide_keys(
    key_lhs: &[u8],
    timestamp_lhs: u64,
    key_rhs: &[u8],
    timestamp_rhs: u64,
) -> (Vec<u8>, u64) {
    assert!(compare_key(key_lhs, timestamp_lhs, key_rhs, timestamp_rhs) == Ordering::Less);
    let max_shared = cmp::min(key_lhs.len(), key_rhs.len());
    let mut shared = 0;
    while shared < max_shared && key_lhs[shared] == key_rhs[shared] {
        shared += 1;
    }
    let mut d_key: Vec<u8> = Vec::with_capacity(shared + 1);
    let d_timestamp: u64;
    if shared < max_shared && key_lhs[shared] + 1 < key_rhs[shared] {
        assert!(key_lhs.len() > shared);
        assert!(key_rhs.len() > shared);
        assert!(key_lhs[shared] < key_rhs[shared]);
        assert!(key_lhs[shared] < 0xff);
        d_key.extend_from_slice(&key_lhs[0..shared+1]);
        d_key[shared] = key_lhs[shared] + 1;
        d_timestamp = 0;
    } else {
        d_key.extend_from_slice(&key_lhs);
        d_timestamp = timestamp_lhs;
    }
    let cmp_lhs = compare_key(key_lhs, timestamp_lhs, &d_key, d_timestamp);
    let cmp_rhs = compare_key(&d_key, d_timestamp, key_rhs, timestamp_rhs);
    assert!(cmp_lhs == Ordering::Less || cmp_lhs == Ordering::Equal);
    assert!(cmp_rhs == Ordering::Less);
    (d_key, d_timestamp)
}

/////////////////////////////////////// minimal_successor_key //////////////////////////////////////

fn minimal_successor_key(key: &[u8], timestamp: u64) -> (Vec<u8>, u64) {
    let mut key = key.to_vec();
    let timestamp = if timestamp == 0 {
        key.push(0);
        0
    } else {
        timestamp - 1
    };
    (key, timestamp)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_ref_ordering() {
        let kvp1 = KeyRef {
            key: "key1".as_bytes(),
            timestamp: 42,
        };
        let kvp2 = KeyRef {
            key: "key1".as_bytes(),
            timestamp: 84,
        };
        let kvp3 = KeyRef {
            key: "key2".as_bytes(),
            timestamp: 99,
        };
        assert!(kvp2 < kvp1);
        assert!(kvp3 > kvp2);
        assert!(kvp3 > kvp1);
    }

    mod divide_keys {
        use super::*;

        #[test]
        fn empty_timestamp() {
            let lhs_key: &[u8] = &[];
            let rhs_key: &[u8] = &[];
            let lhs_timestamp = 2u64;
            let rhs_timestamp = 0u64;
            let (d_key, d_timestamp) = divide_keys(lhs_key, lhs_timestamp, rhs_key, rhs_timestamp);
            let d_key: &[u8] = &d_key;
            assert_eq!(rhs_key, d_key);
            assert_eq!(2, d_timestamp);
        }

        #[test]
        fn empty_timestamp_adjacent() {
            let lhs_key: &[u8] = &[];
            let rhs_key: &[u8] = &[];
            let lhs_timestamp = 1u64;
            let rhs_timestamp = 0u64;
            let (d_key, d_timestamp) = divide_keys(lhs_key, lhs_timestamp, rhs_key, rhs_timestamp);
            let d_key: &[u8] = &d_key;
            assert_eq!(rhs_key, d_key);
            assert_eq!(1, d_timestamp);
        }

        #[test]
        fn empty_one() {
            let lhs_key: &[u8] = &[];
            let rhs_key: &[u8] = &[1];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            assert_eq!(lhs_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn max_timestamp() {
            let lhs_key: &[u8] = &[0xff];
            let rhs_key: &[u8] = &[0xff];
            let lhs_timestamp = 1u64;
            let rhs_timestamp = 0u64;
            let (d_key, d_timestamp) = divide_keys(lhs_key, lhs_timestamp, rhs_key, rhs_timestamp);
            let d_key: &[u8] = &d_key;
            assert_eq!(lhs_key, d_key);
            assert_eq!(1, d_timestamp);
        }

        #[test]
        fn shared_prefix_no_diff() {
            let lhs_key: &[u8] = &[0xaa];
            let rhs_key: &[u8] = &[0xaa, 0xaa];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xaa];
            assert_eq!(exp_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn shared_prefix_0xaa() {
            let lhs_key: &[u8] = &[0xaa, 0x0];
            let rhs_key: &[u8] = &[0xaa, 0x5, 0xaa];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xaa, 0x1];
            assert_eq!(exp_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn shared_prefix_0xff() {
            let lhs_key: &[u8] = &[0xff, 0xff, 0x0];
            let rhs_key: &[u8] = &[0xff, 0xff, 0x5, 0xff, 0xff];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xff, 0xff, 0x1];
            assert_eq!(exp_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn adjacent_shared() {
            let lhs_key: &[u8] = &[0xff, 0xff, 0x0];
            let rhs_key: &[u8] = &[0xff, 0xff, 0x1];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 5);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xff, 0xff, 0x0];
            assert_eq!(exp_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn bug_1() {
            let lhs_key: &[u8] = &[54];
            let rhs_key: &[u8] = &[56];
            let lhs_timestamp = 4025094399440583762;
            let rhs_timestamp = 16919648803326809016;
            let (d_key, d_timestamp) = divide_keys(lhs_key, lhs_timestamp, rhs_key, rhs_timestamp);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[55];
            let exp_timestamp = 0;
            assert_eq!(exp_key, d_key);
            assert_eq!(exp_timestamp, d_timestamp);
        }
    }

    mod minimal_successor_key {
        use super::*;

        #[test]
        fn empty_zero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[], 0);
            let exp: &[u8] = &[0x00];
            assert_eq!(exp, &key);
            assert_eq!(0, timestamp);
        }

        #[test]
        fn empty_nonzero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[], 1);
            let exp: &[u8] = &[];
            assert_eq!(exp, &key);
            assert_eq!(0, timestamp);
        }

        #[test]
        fn nonempty_zero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[0xaa], 0);
            let exp: &[u8] = &[0xaa, 0x00];
            assert_eq!(exp, &key);
            assert_eq!(0, timestamp);
        }

        #[test]
        fn nonempty_nonzero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[0xaa], 5);
            let exp: &[u8] = &[0xaa];
            assert_eq!(exp, &key);
            assert_eq!(4, timestamp);
        }

        #[test]
        fn ffffff_zero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[0xff, 0xff, 0xff], 0);
            let exp: &[u8] = &[0xff, 0xff, 0xff, 0x00];
            assert_eq!(exp, &key);
            assert_eq!(0, timestamp);
        }

        #[test]
        fn ffffff_nonzero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[0xff, 0xff, 0xff], 7);
            let exp: &[u8] = &[0xff, 0xff, 0xff];
            assert_eq!(exp, &key);
            assert_eq!(6, timestamp);
        }
    }
}
