use std::cmp;
use std::cmp::Ordering;

extern crate prototk;
#[macro_use]
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
        Error::BlockError {
            what,
        }
    }
}

/////////////////////////////////////////// KeyValuePair ///////////////////////////////////////////

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
        compare_bytes(key1, key2)
            .then(self.timestamp.cmp(&rhs.timestamp).reverse())
    }
}

impl<'a> PartialOrd for KeyValuePair<'a> {
    fn partial_cmp(&self, rhs: &KeyValuePair) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

///////////////////////////////////////////// Iterator /////////////////////////////////////////////

pub trait Iterator {
    // Seek functions should not return a value, but instead position the cursor so that a
    // subsequent call to next or prev will return the right result.  For example, seek_to_first
    // should position the cursor so that a prev returns None and a next returns the first result.
    // A seek should position the cursor so that a call to next will return the result, while a
    // call to prev will return the previous result.
    fn seek_to_first(&mut self) -> Result<(), Error>;
    fn seek_to_last(&mut self) -> Result<(), Error>;
    fn seek(&mut self, key: &[u8]) -> Result<(), Error>;

    fn prev(&mut self) -> Result<Option<KeyValuePair>, Error>;
    fn next(&mut self) -> Result<Option<KeyValuePair>, Error>;
    fn same(&mut self) -> Result<Option<KeyValuePair>, Error>;
}

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

pub trait KeyValueStore {
    fn get_at_timestamp<'a>(&'a self, key: &[u8], timestamp: u64) -> Option<KeyValuePair<'a>>;

    fn iter<'a>(&'a self) -> Box<dyn Iterator + 'a>;
    fn scan<'a>(&'a self, key: &[u8], timestamp: u64) -> Result<Box<dyn Iterator + 'a>, Error>;

    fn transact<'a>(&'a mut self, timestamp: u64) -> Box<dyn Transaction + 'a>;
}

//////////////////////////////////////////// Transaction ///////////////////////////////////////////

pub trait Transaction: KeyValueStore {
    fn commit(self);
    fn get<'a>(&'a mut self, key: &[u8]) -> Option<KeyValuePair<'a>>;
    fn put(&mut self, key: &[u8], value: &[u8]);
    fn del(&mut self, key: &[u8]);
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

// Content under CC By-Sa.  I just use as is, as can you.
// https://codereview.stackexchange.com/questions/233872/writing-slice-compare-in-a-more-compact-way
pub fn compare_bytes(a: &[u8], b: &[u8]) -> cmp::Ordering {
    for (ai, bi) in a.iter().zip(b.iter()) {
        match ai.cmp(&bi) {
            Ordering::Equal => continue,
            ord => return ord
        }
    }

    /* if every single element was equal, compare length */
    a.len().cmp(&b.len())
}
// End borrowed code

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
        assert!(kvp2 > kvp1);
        assert!(kvp3 > kvp2);
        assert!(kvp3 > kvp1);
    }
}
