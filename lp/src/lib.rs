extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use std::cmp;
use std::cmp::Ordering;

pub mod block;
pub mod buffer;
pub mod file_manager;
pub mod guacamole;
pub mod reference;
pub mod sst;

pub use buffer::Buffer;

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

/////////////////////////////////////// KeyValuePair ///////////////////////////////////////

#[derive(Debug)]
pub struct KeyValuePair {
    pub key: Buffer,
    pub timestamp: u64,
    pub value: Option<Buffer>,
}

impl Eq for KeyValuePair {}

impl PartialEq for KeyValuePair {
    fn eq(&self, rhs: &KeyValuePair) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl Ord for KeyValuePair {
    fn cmp(&self, rhs: &KeyValuePair) -> std::cmp::Ordering {
        let key_lhs: &[u8] = self.key.as_bytes();
        let key_rhs: &[u8] = rhs.key.as_bytes();
        compare_key(key_lhs, self.timestamp, key_rhs, rhs.timestamp)
    }
}

impl PartialOrd for KeyValuePair {
    fn partial_cmp(&self, rhs: &KeyValuePair) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
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

    fn prev(&mut self) -> Result<Option<KeyValuePair>, Error>;
    fn next(&mut self) -> Result<Option<KeyValuePair>, Error>;
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
    // The keys have a shared prefix.  It necessarily means that key_lhs.len() == shared and
    // key_rhs.len() >= shared.
    if shared == max_shared {
        assert!(key_lhs.len() == max_shared);
        assert!(key_rhs.len() >= max_shared);
        if key_lhs.len() == key_rhs.len() {
            // When the keys are equal, fall back to timestamps.
            assert!(timestamp_lhs > timestamp_rhs);
            d_key.extend_from_slice(&key_lhs);
            d_timestamp = timestamp_lhs - 1;
        } else {
            // When the keys are not equal, we know that key_rhs.len() > shared.
            assert!(shared + 1 <= key_rhs.len());
            assert!(key_lhs.len() < key_rhs.len());
            d_key.extend_from_slice(&key_rhs[0..shared + 1]);
            // When key_rhs is one byte longer, use timestamp_rhs; else use timestamp 0.
            d_timestamp = if key_lhs.len() + 1 == key_rhs.len() {
                timestamp_rhs
            } else {
                0
            }
        }
    } else {
        // We know we can divide the keys at a byte less than key_lhs.len() and key_rhs.len().
        assert!(key_lhs.len() > shared);
        assert!(key_rhs.len() > shared);
        if key_rhs.len() == shared + 1 && key_lhs[shared] + 1 == key_rhs[shared] {
            // We have a special case where key_rhs is short and adjacent to key_lhs.  Use key_rhs
            // with its timestamp.
            d_key.extend_from_slice(&key_rhs);
            d_timestamp = timestamp_rhs;
        } else {
            // Use a prefix of key_rhs with a zero timestamp.
            d_key.extend_from_slice(&key_rhs[..shared + 1]);
            d_timestamp = 0;
        }
    }
    (d_key, d_timestamp)
}

/////////////////////////////////////// minimal_successor_key //////////////////////////////////////

fn minimal_successor_key(key: &[u8], timestamp: u64) -> (Vec<u8>, u64) {
    let all_ff = key.iter().all(|x| *x == 0xffu8);
    let (sz, ts) = if all_ff && timestamp == 0 {
        (key.len() + 1, 0)
    } else if all_ff {
        (key.len(), timestamp - 1)
    } else {
        (key.len(), 0)
    };
    let mut key = Vec::with_capacity(sz);
    key.resize(sz, 0xff);
    (key, ts)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_value_pair_ordering() {
        let kvp1 = KeyValuePair {
            key: "key1".into(),
            timestamp: 42,
            value: Some("value".into()),
        };
        let kvp2 = KeyValuePair {
            key: "key1".into(),
            timestamp: 84,
            value: Some("value".into()),
        };
        let kvp3 = KeyValuePair {
            key: "key2".into(),
            timestamp: 99,
            value: Some("value".into()),
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
            assert_eq!(1, d_timestamp);
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
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn empty_one() {
            let lhs_key: &[u8] = &[];
            let rhs_key: &[u8] = &[1];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            assert_eq!(rhs_key, d_key);
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
            assert_eq!(rhs_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn shared_prefix_no_diff() {
            let lhs_key: &[u8] = &[0xaa];
            let rhs_key: &[u8] = &[0xaa, 0xaa];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xaa, 0xaa];
            assert_eq!(exp_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn shared_prefix_0xaa() {
            let lhs_key: &[u8] = &[0xaa, 0x0];
            let rhs_key: &[u8] = &[0xaa, 0x5, 0xaa];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xaa, 0x5];
            assert_eq!(exp_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn shared_prefix_0xff() {
            let lhs_key: &[u8] = &[0xff, 0xff, 0x0];
            let rhs_key: &[u8] = &[0xff, 0xff, 0x5, 0xff, 0xff];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 0);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xff, 0xff, 0x5];
            assert_eq!(exp_key, d_key);
            assert_eq!(0, d_timestamp);
        }

        #[test]
        fn use_rhs() {
            let lhs_key: &[u8] = &[0xff, 0xff, 0x0];
            let rhs_key: &[u8] = &[0xff, 0xff, 0x1];
            let (d_key, d_timestamp) = divide_keys(lhs_key, 0, rhs_key, 5);
            let d_key: &[u8] = &d_key;
            let exp_key: &[u8] = &[0xff, 0xff, 0x1];
            assert_eq!(exp_key, d_key);
            assert_eq!(5, d_timestamp);
        }
    }

    mod minimal_successor_key {
        use super::*;

        #[test]
        fn empty_zero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[], 0);
            let exp: &[u8] = &[0xff];
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
            let exp: &[u8] = &[0xff];
            assert_eq!(exp, &key);
            assert_eq!(0, timestamp);
        }

        #[test]
        fn nonempty_nonzero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[0xaa], 5);
            let exp: &[u8] = &[0xff];
            assert_eq!(exp, &key);
            assert_eq!(0, timestamp);
        }

        #[test]
        fn ffffff_zero_timestamp() {
            let (key, timestamp) = minimal_successor_key(&[0xff, 0xff, 0xff], 0);
            let exp: &[u8] = &[0xff, 0xff, 0xff, 0xff];
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
