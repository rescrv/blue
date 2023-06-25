extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use std::cmp;
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;

use buffertk::Buffer;

use biometrics::Counter;

use tatl::{HeyListen, Stationary};

use zerror::Z;
use zerror_core::ErrorCore;

pub mod cli;
pub mod db;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOGIC_ERROR: Counter = Counter::new("lp.logic_error");
static LOGIC_ERROR_MONITOR: Stationary = Stationary::new("lp.logic_error", &LOGIC_ERROR);

static CORRUPTION: Counter = Counter::new("lp.corruption");
static CORRUPTION_MONITOR: Stationary = Stationary::new("lp.corruption", &CORRUPTION);

static KEY_TOO_LARGE: Counter = Counter::new("lp.error.key_too_large");
static KEY_TOO_LARGE_MONITOR: Stationary = Stationary::new("lp.error.key_too_large", &KEY_TOO_LARGE);

static VALUE_TOO_LARGE: Counter = Counter::new("lp.error.value_too_large");
static VALUE_TOO_LARGE_MONITOR: Stationary = Stationary::new("lp.error.value_too_large", &VALUE_TOO_LARGE);

static TABLE_FULL: Counter = Counter::new("lp.error.table_full");
static TABLE_FULL_MONITOR: Stationary = Stationary::new("lp.error.table_full", &TABLE_FULL);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOGIC_ERROR_MONITOR);
    hey_listen.register_stationary(&CORRUPTION_MONITOR);
    hey_listen.register_stationary(&KEY_TOO_LARGE_MONITOR);
    hey_listen.register_stationary(&VALUE_TOO_LARGE_MONITOR);
    hey_listen.register_stationary(&TABLE_FULL_MONITOR);

    db::register_monitors(hey_listen);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    KeyTooLarge {
        core: ErrorCore,
        length: usize,
        limit: usize,
    },
    ValueTooLarge {
        core: ErrorCore,
        length: usize,
        limit: usize,
    },
    SortOrder {
        core: ErrorCore,
        last_key: Vec<u8>,
        last_timestamp: u64,
        new_key: Vec<u8>,
        new_timestamp: u64,
    },
    TableFull {
        core: ErrorCore,
        size: usize,
        limit: usize,
    },
    BlockTooSmall {
        core: ErrorCore,
        length: usize,
        required: usize,
    },
    UnpackError {
        core: ErrorCore,
        error: prototk::Error,
        context: String,
    },
    CRC32CFailure {
        core: ErrorCore,
        start: u64,
        limit: u64,
        crc32c: u32,
    },
    LockNotObtained {
        core: ErrorCore,
        path: PathBuf,
    },
    DuplicateSST {
        core: ErrorCore,
        what: String,
    },
    Corruption {
        core: ErrorCore,
        context: String,
    },
    LogicError {
        core: ErrorCore,
        context: String,
    },
    SystemError {
        core: ErrorCore,
        context: String,
    },
    IOError {
        core: ErrorCore,
        what: std::io::Error,
    },
    TooManyOpenFiles {
        core: ErrorCore,
        limit: usize,
    },
    SSTNotFound {
        core: ErrorCore,
        setsum: String,
    },
    DBExists {
        core: ErrorCore,
        path: PathBuf,
    },
    DBNotExist {
        core: ErrorCore,
        path: PathBuf,
    },
    PathError {
        core: ErrorCore,
        path: PathBuf,
        what: String,
    },
    MissingManifest {
        core: ErrorCore,
        path: PathBuf,
    },
    MissingSST {
        core: ErrorCore,
        path: PathBuf,
    },
    ExtraFile {
        core: ErrorCore,
        path: PathBuf,
    },
    InvalidManifestLine {
        core: ErrorCore,
        line: String,
    },
    InvalidManifestCommand {
        core: ErrorCore,
        cmd: String,
        arg: String,
    },
    InvalidManifestSetsum {
        core: ErrorCore,
        manifest: String,
        computed: String,
    },
    InvalidSSTSetsum {
        core: ErrorCore,
        expected: String,
        computed: String,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::CRC32CFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSST { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::IOError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SSTNotFound { core, .. } => { core } ,
            Error::DBExists { core, .. } => { core } ,
            Error::DBNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
            Error::MissingManifest { core, .. } => { core } ,
            Error::MissingSST { core, .. } => { core } ,
            Error::ExtraFile { core, .. } => { core } ,
            Error::InvalidManifestLine { core, .. } => { core } ,
            Error::InvalidManifestCommand { core, .. } => { core } ,
            Error::InvalidManifestSetsum { core, .. } => { core } ,
            Error::InvalidSSTSetsum { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::CRC32CFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSST { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::IOError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SSTNotFound { core, .. } => { core } ,
            Error::DBExists { core, .. } => { core } ,
            Error::DBNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
            Error::MissingManifest { core, .. } => { core } ,
            Error::MissingSST { core, .. } => { core } ,
            Error::ExtraFile { core, .. } => { core } ,
            Error::InvalidManifestLine { core, .. } => { core } ,
            Error::InvalidManifestCommand { core, .. } => { core } ,
            Error::InvalidManifestSetsum { core, .. } => { core } ,
            Error::InvalidSSTSetsum { core, .. } => { core } ,
        }
    }
}

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}", self) + "\n" + &self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.set_token(identifier, value);
        self
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        self.core_mut().set_token(identifier, value);
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.core_mut().set_url(identifier, url);
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error where X: Debug {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.core_mut().set_variable(variable, x);
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        // TODO(rescrv):  Make sure this isn't infinitely co-recursive with long_form
        write!(fmt, "{}", self.long_form())
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::IOError { core: ErrorCore::default(), what }
    }
}

////////////////////////////////////////////// FromIO //////////////////////////////////////////////

pub trait FromIO {
    type Result;

    fn from_io(self) -> Self::Result;
}

impl<T> FromIO for Result<T, std::io::Error> {
    type Result = Result<T, Error>;

    fn from_io(self) -> Self::Result {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(Error::from(e)),
        }
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
            },
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

/////////////////////////////////////////// TableMetadata //////////////////////////////////////////

pub trait TableMetadata {
    fn first_key(&self) -> KeyRef;
    fn last_key(&self) -> KeyRef;
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

pub trait Cursor {
    fn seek_to_first(&mut self) -> Result<(), Error>;
    fn seek_to_last(&mut self) -> Result<(), Error>;
    fn seek(&mut self, key: &[u8]) -> Result<(), Error>;

    fn prev(&mut self) -> Result<(), Error>;
    fn next(&mut self) -> Result<(), Error>;

    fn key(&self) -> Option<KeyRef>;
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

    mod crc32c {
        // Tests of crc32c borrowed from the LevelDB library.  Used to track upstream.
        //
        // Copyright (c) 2011 The LevelDB Authors. All rights reserved.
        // Use of this source code is governed by a BSD-style license that can be
        // found in the LICENSE file. See the AUTHORS file for names of contributors.

        #[test]
        fn standard_results() {
            // Test copied directly from LevelDB.
            // From rfc3720 section B.4.

            let buf: [u8; 32] = [0u8; 32];
            assert_eq!(0x8a9136aa, crc32c::crc32c(&buf));

            let buf: [u8; 32] = [0xffu8; 32];
            assert_eq!(0x62a8ab43, crc32c::crc32c(&buf));

            let mut buf: [u8; 32] = [0; 32];
            for i in 0..32 {
                buf[i] = i as u8;
            }
            assert_eq!(0x46dd794e, crc32c::crc32c(&buf));

            for i in 0..32 {
                buf[i] = 31 - i as u8;
            }
            assert_eq!(0x113fdb5c, crc32c::crc32c(&buf));

            let data: [u8; 48] = [
                0x01, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x14,
                0x00, 0x00, 0x00, 0x18, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];
            assert_eq!(0xd9963a56, crc32c::crc32c(&data));
        }

        #[test]
        fn values() {
            assert_ne!(
                crc32c::crc32c("a".as_bytes()),
                crc32c::crc32c("foo".as_bytes())
            );
        }

        #[test]
        fn extends() {
            assert_eq!(
                crc32c::crc32c("hello world".as_bytes()),
                crc32c::crc32c_append(crc32c::crc32c("hello ".as_bytes()), "world".as_bytes())
            );
        }
    }
}
