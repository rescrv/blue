extern crate prototk;
extern crate prototk_derive;

use std::cmp;
use std::cmp::Ordering;

pub mod block;
pub mod reference;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
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
}

/////////////////////////////////////// KeyValuePair ///////////////////////////////////////

#[derive(Debug, Eq)]
pub struct KeyValuePair<'a> {
    pub key: &'a [u8],
    pub timestamp: u64,
    pub value: Option<&'a [u8]>,
}

impl<'a> PartialEq for KeyValuePair<'a> {
    fn eq(&self, rhs: &KeyValuePair) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl<'a> Ord for KeyValuePair<'a> {
    fn cmp(&self, rhs: &KeyValuePair) -> std::cmp::Ordering {
        let key1 = self.key;
        let key2 = rhs.key;
        compare_key(key1, self.timestamp, key2, rhs.timestamp)
    }
}

impl<'a> PartialOrd for KeyValuePair<'a> {
    fn partial_cmp(&self, rhs: &KeyValuePair) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

/////////////////////////////////////////////// Table //////////////////////////////////////////////

pub trait TableTrait<'a> {
    type Builder: TableBuilderTrait<'a, Table = Self>;
    type Cursor: TableCursorTrait<'a>;
    fn iterate(&'a self) -> Self::Cursor;
}

/////////////////////////////////////////// TableBuilder ///////////////////////////////////////////

pub trait TableBuilderTrait<'a> {
    type Table: TableTrait<'a>;

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error>;
    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error>;

    fn seal(self) -> Result<Self::Table, Error>;
}

//////////////////////////////////////////// TableCursor ///////////////////////////////////////////

pub trait TableCursorTrait<'a> {
    fn get(&mut self, key: &[u8], timestamp: u64) -> Result<Option<KeyValuePair>, Error> {
        self.seek(key, timestamp)?;
        match self.next()? {
            Some(kvp) => {
                if compare_bytes(kvp.key, key) == Ordering::Equal {
                    Ok(Some(KeyValuePair {
                        key: kvp.key,
                        timestamp: kvp.timestamp,
                        value: match kvp.value {
                            Some(v) => Some(&v),
                            None => None,
                        },
                    }))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }


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
            key: "key1".as_bytes(),
            timestamp: 42,
            value: Some("value".as_bytes()),
        };
        let kvp2 = KeyValuePair {
            key: "key1".as_bytes(),
            timestamp: 84,
            value: Some("value".as_bytes()),
        };
        let kvp3 = KeyValuePair {
            key: "key2".as_bytes(),
            timestamp: 99,
            value: Some("value".as_bytes()),
        };
        assert!(kvp2 < kvp1);
        assert!(kvp3 > kvp2);
        assert!(kvp3 > kvp1);
    }

    struct TestTable {}

    impl<'a> TableTrait<'a> for TestTable {
        type Builder = TestBuilder;
        type Cursor = TestCursor;

        fn iterate(&self) -> Self::Cursor {
            unimplemented!();
        }
    }

    struct TestBuilder {}

    impl<'a> TableBuilderTrait<'a> for TestBuilder {
        type Table = TestTable;

        fn put(&mut self, _key: &[u8], _timestamp: u64, _value: &[u8]) -> Result<(), Error> {
            unimplemented!();
        }

        fn del(&mut self, _key: &[u8], _timestamp: u64) -> Result<(), Error> {
            unimplemented!();
        }

        fn seal(self) -> Result<TestTable, Error> {
            unimplemented!();
        }
    }

    struct TestCursor {}

    impl<'a> TableCursorTrait<'a> for TestCursor {
        fn get(&mut self, _key: &[u8], _timestamp: u64) -> Result<Option<KeyValuePair>, Error> {
            unimplemented!();
        }

        fn seek_to_first(&mut self) -> Result<(), Error> {
            unimplemented!();
        }

        fn seek_to_last(&mut self) -> Result<(), Error> {
            unimplemented!();
        }

        fn seek(&mut self, _key: &[u8], _timestamp: u64) -> Result<(), Error> {
            unimplemented!();
        }

        fn prev(&mut self) -> Result<Option<KeyValuePair>, Error> {
            unimplemented!();
        }

        fn next(&mut self) -> Result<Option<KeyValuePair>, Error> {
            unimplemented!();
        }
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
