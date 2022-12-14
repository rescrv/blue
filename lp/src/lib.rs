use std::cmp;
use std::cmp::Ordering;

extern crate prototk;
extern crate prototk_derive;

pub mod block;
pub mod reference;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    BlockError { what: block::Error },
}

impl From<block::Error> for Error {
    fn from(what: block::Error) -> Error {
        Error::BlockError { what }
    }
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
        compare_bytes(key1, key2).then(self.timestamp.cmp(&rhs.timestamp).reverse())
    }
}

impl<'a> PartialOrd for KeyValuePair<'a> {
    fn partial_cmp(&self, rhs: &KeyValuePair) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

/////////////////////////////////////////////// Table //////////////////////////////////////////////

pub trait Table<'a> {
    type Builder: TableBuilder<'a, Table = Self>;
    type Cursor: TableCursor<'a>;

    fn get(&'a self, key: &[u8], timestamp: u64) -> Option<KeyValuePair<'a>>;
    fn iterate(&'a self) -> Self::Cursor;
}

/////////////////////////////////////////// TableBuilder ///////////////////////////////////////////

pub trait TableBuilder<'a> {
    type Table: Table<'a>;

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error>;
    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error>;

    fn seal(self) -> Result<Self::Table, Error>;
}

//////////////////////////////////////////// TableCursor ///////////////////////////////////////////

pub trait TableCursor<'a> {
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

pub fn compare_key(key_lhs: &[u8], timestamp_lhs: u64, key_rhs: &[u8], timestamp_rhs: u64) -> Ordering {
    compare_bytes(key_lhs, key_rhs).then(timestamp_lhs.cmp(&timestamp_rhs).reverse())
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

    impl<'a> Table<'a> for TestTable {
        type Builder = TestBuilder;
        type Cursor = TestCursor;

        fn get(&self, _key: &[u8], _timestamp: u64) -> Option<KeyValuePair<'a>> {
            unimplemented!();
        }

        fn iterate(&self) -> Self::Cursor {
            unimplemented!();
        }
    }

    struct TestBuilder {}

    impl<'a> TableBuilder<'a> for TestBuilder {
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

    impl<'a> TableCursor<'a> for TestCursor {
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
}
