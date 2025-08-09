//! sst stands for sorted-string-table.
//!
//! This crate provides an implementation of an SST and most common cursoring patterns necessary to
//! create something like a log-structured merge tree out of SSTs.

extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use std::cmp;
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::ops::Bound;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use biometrics::Counter;
use buffertk::{stack_pack, Packable, Unpacker};
use tatl::{HeyListen, Stationary};
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

pub mod block;
pub mod bounds_cursor;
pub mod concat_cursor;
pub mod file_manager;
pub mod gc;
pub mod ingest;
pub mod lazy_cursor;
pub mod log;
pub mod merging_cursor;
pub mod pruning_cursor;
pub mod reference;
pub mod sbbf;
pub mod setsum;

pub use log::{LogBuilder, LogIterator, LogOptions};
pub use setsum::Setsum;

use block::{Block, BlockBuilder, BlockBuilderOptions, BlockCursor};
use bounds_cursor::BoundsCursor;
use file_manager::{open_without_manager, FileHandle};
use pruning_cursor::PruningCursor;
use sbbf::Filter;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOGIC_ERROR: Counter = Counter::new("sst.logic_error");
static LOGIC_ERROR_MONITOR: Stationary = Stationary::new("sst.logic_error", &LOGIC_ERROR);

static CORRUPTION: Counter = Counter::new("sst.corruption");
static CORRUPTION_MONITOR: Stationary = Stationary::new("sst.corruption", &CORRUPTION);

static KEY_TOO_LARGE: Counter = Counter::new("sst.error.key_too_large");
static KEY_TOO_LARGE_MONITOR: Stationary =
    Stationary::new("sst.error.key_too_large", &KEY_TOO_LARGE);

static VALUE_TOO_LARGE: Counter = Counter::new("sst.error.value_too_large");
static VALUE_TOO_LARGE_MONITOR: Stationary =
    Stationary::new("sst.error.value_too_large", &VALUE_TOO_LARGE);

static SORT_ORDER: Counter = Counter::new("sst.error.SORT_ORDER");
static SORT_ORDER_MONITOR: Stationary = Stationary::new("sst.error.SORT_ORDER", &SORT_ORDER);

static TABLE_FULL: Counter = Counter::new("sst.error.table_full");
static TABLE_FULL_MONITOR: Stationary = Stationary::new("sst.error.table_full", &TABLE_FULL);

static BLOCK_TOO_SMALL: Counter = Counter::new("sst.error.block_too_small");
static BLOCK_TOO_SMALL_MONITOR: Stationary =
    Stationary::new("sst.error.block_too_small", &BLOCK_TOO_SMALL);

static UNPACK_ERROR: Counter = Counter::new("sst.error.unpack_error");
static UNPACK_ERROR_MONITOR: Stationary = Stationary::new("sst.error.unpack_error", &UNPACK_ERROR);

static CRC32C_FAILURE: Counter = Counter::new("sst.error.crc32c_failure");
static CRC32C_FAILURE_MONITOR: Stationary =
    Stationary::new("sst.error.crc32c_failure", &CRC32C_FAILURE);

static SYSTEM_ERROR: Counter = Counter::new("sst.error.system_error");
static SYSTEM_ERROR_MONITOR: Stationary = Stationary::new("sst.error.system_error", &SYSTEM_ERROR);

static TOO_MANY_OPEN_FILES: Counter = Counter::new("sst.error.too_many_open_files");
static TOO_MANY_OPEN_FILES_MONITOR: Stationary =
    Stationary::new("sst.error.too_many_open_files", &TOO_MANY_OPEN_FILES);

static EMPTY_BATCH: Counter = Counter::new("sst.error.empty_batch");
static EMPTY_BATCH_MONITOR: Stationary = Stationary::new("sst.error.empty_batch", &EMPTY_BATCH);

static SST_OPEN: Counter = Counter::new("sst.table.open");
static SST_SETSUM: Counter = Counter::new("sst.table.setsum");
static SST_METADATA: Counter = Counter::new("sst.table.metadata");
static SST_LOAD_BLOCK: Counter = Counter::new("sst.table.load_block");
static SST_LOAD_FILTER: Counter = Counter::new("sst.table.load_filter");
static BUILDER_NEW: Counter = Counter::new("sst.builder.new");
static BUILDER_COMPARE_KEY: Counter = Counter::new("sst.builder.compare_key");
static BUILDER_ASSIGN_LAST_KEY: Counter = Counter::new("sst.builder.assign_last_key");
static BUILDER_START_NEW_BLOCK: Counter = Counter::new("sst.builder.start_new_block");
static BUILDER_FLUSH_BLOCK: Counter = Counter::new("sst.builder.flush_block");
static BUILDER_APPROX_SIZE: Counter = Counter::new("sst.builder.approx_size");
static BUILDER_PUT: Counter = Counter::new("sst.builder.put");
static BUILDER_DEL: Counter = Counter::new("sst.builder.del");
static BUILDER_SEAL: Counter = Counter::new("sst.builder.seal");
static SST_BLOOM_NEGATIVE: Counter = Counter::new("sst.bloom.negative");
static SST_BLOOM_FALSE_POSITIVE: Counter = Counter::new("sst.bloom.false_positive");
static SST_CURSOR_META_PREV: Counter = Counter::new("sst.cursor.meta_prev");
static SST_CURSOR_META_NEXT: Counter = Counter::new("sst.cursor.meta_next");
static SST_CURSOR_RESET: Counter = Counter::new("sst.cursor.reset");
static SST_CURSOR_SEEK_TO_FIRST: Counter = Counter::new("sst.cursor.seek_to_first");
static SST_CURSOR_SEEK_TO_LAST: Counter = Counter::new("sst.cursor.seek_to_last");
static SST_CURSOR_SEEK: Counter = Counter::new("sst.cursor.seek");
static SST_CURSOR_PREV: Counter = Counter::new("sst.cursor.prev");
static SST_CURSOR_NEXT: Counter = Counter::new("sst.cursor.next");
static SST_CURSOR_NEW: Counter = Counter::new("sst.cursor.new");

/// Register this crate's biometrics.
pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&LOGIC_ERROR);
    collector.register_counter(&CORRUPTION);
    collector.register_counter(&KEY_TOO_LARGE);
    collector.register_counter(&VALUE_TOO_LARGE);
    collector.register_counter(&SORT_ORDER);
    collector.register_counter(&TABLE_FULL);
    collector.register_counter(&BLOCK_TOO_SMALL);
    collector.register_counter(&UNPACK_ERROR);
    collector.register_counter(&CRC32C_FAILURE);
    collector.register_counter(&SYSTEM_ERROR);
    collector.register_counter(&TOO_MANY_OPEN_FILES);
    collector.register_counter(&EMPTY_BATCH);
    collector.register_counter(&SST_OPEN);
    collector.register_counter(&SST_CURSOR_NEW);
    collector.register_counter(&SST_SETSUM);
    collector.register_counter(&SST_METADATA);
    collector.register_counter(&SST_LOAD_BLOCK);
    collector.register_counter(&SST_LOAD_FILTER);
    collector.register_counter(&BUILDER_NEW);
    collector.register_counter(&BUILDER_COMPARE_KEY);
    collector.register_counter(&BUILDER_ASSIGN_LAST_KEY);
    collector.register_counter(&BUILDER_START_NEW_BLOCK);
    collector.register_counter(&BUILDER_FLUSH_BLOCK);
    collector.register_counter(&BUILDER_APPROX_SIZE);
    collector.register_counter(&BUILDER_PUT);
    collector.register_counter(&BUILDER_DEL);
    collector.register_counter(&BUILDER_SEAL);
    collector.register_counter(&SST_BLOOM_NEGATIVE);
    collector.register_counter(&SST_BLOOM_FALSE_POSITIVE);
    collector.register_counter(&SST_CURSOR_META_PREV);
    collector.register_counter(&SST_CURSOR_META_NEXT);
    collector.register_counter(&SST_CURSOR_RESET);
    collector.register_counter(&SST_CURSOR_SEEK_TO_FIRST);
    collector.register_counter(&SST_CURSOR_SEEK_TO_LAST);
    collector.register_counter(&SST_CURSOR_SEEK);
    collector.register_counter(&SST_CURSOR_PREV);
    collector.register_counter(&SST_CURSOR_NEXT);
    collector.register_counter(&SST_CURSOR_NEW);

    file_manager::register_biometrics(collector);
    log::register_biometrics(collector);
}

/// Register this crate's monitors.
pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOGIC_ERROR_MONITOR);
    hey_listen.register_stationary(&CORRUPTION_MONITOR);
    hey_listen.register_stationary(&KEY_TOO_LARGE_MONITOR);
    hey_listen.register_stationary(&VALUE_TOO_LARGE_MONITOR);
    hey_listen.register_stationary(&SORT_ORDER_MONITOR);
    hey_listen.register_stationary(&TABLE_FULL_MONITOR);
    hey_listen.register_stationary(&BLOCK_TOO_SMALL_MONITOR);
    hey_listen.register_stationary(&UNPACK_ERROR_MONITOR);
    hey_listen.register_stationary(&CRC32C_FAILURE_MONITOR);
    hey_listen.register_stationary(&SYSTEM_ERROR_MONITOR);
    hey_listen.register_stationary(&TOO_MANY_OPEN_FILES_MONITOR);
    hey_listen.register_stationary(&EMPTY_BATCH_MONITOR);

    file_manager::register_monitors(hey_listen);
}

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

/// Check that the key is of valid length, or return a descriptive error.
pub fn check_key_len(key: &[u8]) -> Result<(), Error> {
    if key.len() > MAX_KEY_LEN {
        KEY_TOO_LARGE.click();
        let err = Error::KeyTooLarge {
            core: ErrorCore::default(),
            length: key.len(),
            limit: MAX_KEY_LEN,
        };
        Err(err)
    } else {
        Ok(())
    }
}

/// Check that the value is of valid length, or return a descriptive error.
pub fn check_value_len(value: &[u8]) -> Result<(), Error> {
    if value.len() > MAX_VALUE_LEN {
        VALUE_TOO_LARGE.click();
        let err = Error::ValueTooLarge {
            core: ErrorCore::default(),
            length: value.len(),
            limit: MAX_VALUE_LEN,
        };
        Err(err)
    } else {
        Ok(())
    }
}

/// Check that the table size is allowable, or return a descriptive error.
pub fn check_table_size(size: usize) -> Result<(), Error> {
    if size >= TABLE_FULL_SIZE {
        TABLE_FULL.click();
        let err = Error::TableFull {
            core: ErrorCore::default(),
            size,
            limit: TABLE_FULL_SIZE,
        };
        Err(err)
    } else {
        Ok(())
    }
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// The sst Error type.
#[derive(Clone, Message, zerror_derive::Z)]
pub enum Error {
    /// Success.  Used for Message default.  Should not be constructed otherwise.
    #[prototk(442368, message)]
    Success {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
    },
    /// Indicates the key length is too big for sst.
    #[prototk(442369, message)]
    KeyTooLarge {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The length of the key.
        #[prototk(2, uint64)]
        length: usize,
        /// The limit on length of the key.
        #[prototk(3, uint64)]
        limit: usize,
    },
    /// Indicates the value length is too big for sst.
    #[prototk(442370, message)]
    ValueTooLarge {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The length of the value.
        #[prototk(2, uint64)]
        length: usize,
        /// The limit on length of the value.
        #[prototk(3, uint64)]
        limit: usize,
    },
    /// The SST was provided keys out of order.
    #[prototk(442371, message)]
    SortOrder {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The most recently inserted key.
        #[prototk(2, bytes)]
        last_key: Vec<u8>,
        /// The most recently inserted timestamp.
        #[prototk(3, uint64)]
        last_timestamp: u64,
        /// The key that happened out of order.
        #[prototk(4, bytes)]
        new_key: Vec<u8>,
        /// The timestamp that happened out of order.
        #[prototk(5, uint64)]
        new_timestamp: u64,
    },
    /// The table is full.
    #[prototk(442372, message)]
    TableFull {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The attempted size of the table.
        #[prototk(2, uint64)]
        size: usize,
        /// The limit on size of the table.
        #[prototk(3, uint64)]
        limit: usize,
    },
    /// The block was too small to be considered valid.
    #[prototk(442373, message)]
    BlockTooSmall {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The length observed.
        #[prototk(2, uint64)]
        length: usize,
        /// The length required.
        #[prototk(3, uint64)]
        required: usize,
    },
    /// There was an error unpacking data.
    #[prototk(442374, message)]
    UnpackError {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The prototk unpack error.
        #[prototk(2, message)]
        error: prototk::Error,
        /// Additional context.
        #[prototk(3, string)]
        context: String,
    },
    /// A block failed its crc check.
    #[prototk(442375, message)]
    Crc32cFailure {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The starting offset.
        #[prototk(2, uint64)]
        start: u64,
        /// The limit offset.
        #[prototk(3, uint64)]
        limit: u64,
        /// The computed crc.
        #[prototk(3, fixed32)]
        crc32c: u32,
    },
    /// General corruption was observed.
    #[prototk(442376, message)]
    Corruption {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A description of what was corrupt.
        #[prototk(2, string)]
        context: String,
    },
    /// A logic error was encountered.
    #[prototk(442377, message)]
    LogicError {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A hint as to what went wrong.
        #[prototk(2, string)]
        context: String,
    },
    /// A system error was encountered.
    #[prototk(442378, message)]
    SystemError {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// A hint as to what went wrong.
        #[prototk(2, string)]
        what: String,
    },
    /// Too many files were opened at once.
    #[prototk(442379, message)]
    TooManyOpenFiles {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
        /// The limit on the number of files allowed.
        #[prototk(2, uint64)]
        limit: usize,
    },
    /// An empty batch was provided to a builder that cannot accept empty batches.
    #[prototk(442380, message)]
    EmptyBatch {
        /// The error core.
        #[prototk(1, message)]
        core: ErrorCore,
    },
}

impl Default for Error {
    fn default() -> Self {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError {
            core: ErrorCore::default(),
            what: format!("{what:?}"),
        }
    }
}

impl From<buffertk::Error> for Error {
    fn from(error: buffertk::Error) -> Error {
        let err: prototk::Error = error.into();
        Error::from(err)
    }
}

impl From<prototk::Error> for Error {
    fn from(error: prototk::Error) -> Error {
        Error::UnpackError {
            core: ErrorCore::default(),
            error,
            context: "From<prototk::Error>".to_owned(),
        }
    }
}

impl From<std::convert::Infallible> for Error {
    fn from(_: std::convert::Infallible) -> Error {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

iotoz! {Error}

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

impl<'a> KeyRef<'a> {
    pub fn new(key: &'a [u8], timestamp: u64) -> Self {
        Self { key, timestamp }
    }
}

impl Eq for KeyRef<'_> {}

impl PartialEq for KeyRef<'_> {
    fn eq(&self, rhs: &KeyRef) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl Ord for KeyRef<'_> {
    fn cmp(&self, rhs: &KeyRef) -> std::cmp::Ordering {
        self.key
            .cmp(rhs.key)
            .then(self.timestamp.cmp(&rhs.timestamp).reverse())
    }
}

impl PartialOrd for KeyRef<'_> {
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

impl Display for KeyValueRef<'_> {
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

impl Eq for KeyValueRef<'_> {}

impl PartialEq for KeyValueRef<'_> {
    fn eq(&self, rhs: &KeyValueRef) -> bool {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.eq(&rhs)
    }
}

impl Ord for KeyValueRef<'_> {
    fn cmp(&self, rhs: &KeyValueRef) -> std::cmp::Ordering {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.cmp(&rhs)
    }
}

impl PartialOrd for KeyValueRef<'_> {
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

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

/// A Cursor allows for iterating through data.
pub trait Cursor {
    /// Seek past the first valid key-value pair to a beginning-of-stream sentinel.
    fn seek_to_first(&mut self) -> Result<(), Error>;

    /// Seek past the last valid key-value pair to an end-of-stream sentinel.
    fn seek_to_last(&mut self) -> Result<(), Error>;

    /// Seek to this key.  After a call to seek, the values of [key] and [value] should return the
    /// sought-to key or the key that's lexicographically next after key.
    fn seek(&mut self, key: &[u8]) -> Result<(), Error>;

    /// Advance the cursor forward to the lexicographically-previous key.
    fn prev(&mut self) -> Result<(), Error>;

    /// Advance the cursor forward to the lexicographically-next key.
    fn next(&mut self) -> Result<(), Error>;

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
    fn seek_to_first(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn seek(&mut self, _: &[u8]) -> Result<(), Error> {
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn next(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        None
    }

    fn value(&self) -> Option<&'_ [u8]> {
        None
    }
}

impl Cursor for Box<dyn Cursor> {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.as_mut().seek_to_first()
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.as_mut().seek_to_last()
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        self.as_mut().seek(key)
    }

    fn prev(&mut self) -> Result<(), Error> {
        self.as_mut().prev()
    }

    fn next(&mut self) -> Result<(), Error> {
        self.as_mut().next()
    }

    fn key(&self) -> Option<KeyRef> {
        self.as_ref().key()
    }

    fn value(&self) -> Option<&'_ [u8]> {
        self.as_ref().value()
    }
}

/////////////////////////////////////////// KeyValueEntry //////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum KeyValueEntry<'a> {
    #[prototk(8, message)]
    Put(KeyValuePut<'a>),
    #[prototk(9, message)]
    Del(KeyValueDel<'a>),
}

impl<'a> KeyValueEntry<'a> {
    fn shared(&self) -> usize {
        match self {
            KeyValueEntry::Put(x) => x.shared as usize,
            KeyValueEntry::Del(x) => x.shared as usize,
        }
    }

    fn key_frag(&self) -> &'a [u8] {
        match self {
            KeyValueEntry::Put(x) => x.key_frag,
            KeyValueEntry::Del(x) => x.key_frag,
        }
    }

    fn timestamp(&self) -> u64 {
        match self {
            KeyValueEntry::Put(x) => x.timestamp,
            KeyValueEntry::Del(x) => x.timestamp,
        }
    }

    fn value(&self) -> Option<&'a [u8]> {
        match self {
            KeyValueEntry::Put(x) => Some(x.value),
            KeyValueEntry::Del(_) => None,
        }
    }
}

impl Default for KeyValueEntry<'_> {
    fn default() -> Self {
        Self::Put(KeyValuePut::default())
    }
}

//////////////////////////////////////////// KeyValuePut ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct KeyValuePut<'a> {
    #[prototk(1, uint64)]
    shared: u64,
    #[prototk(2, bytes)]
    key_frag: &'a [u8],
    #[prototk(3, uint64)]
    timestamp: u64,
    #[prototk(4, bytes)]
    value: &'a [u8],
}

//////////////////////////////////////////// KeyValueDel ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct KeyValueDel<'a> {
    #[prototk(5, uint64)]
    shared: u64,
    #[prototk(6, bytes)]
    key_frag: &'a [u8],
    #[prototk(7, uint64)]
    timestamp: u64,
}

////////////////////////////////////////////// Builder /////////////////////////////////////////////

/// A Builder is a generic way of building a sorted (sst) or unsorted (log) string table.
pub trait Builder {
    /// The type that gets returned from seal.
    type Sealed;

    /// The approximate size of the builder.
    fn approximate_size(&self) -> usize;

    /// Put a key into the builder.
    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error>;
    /// Put a tombstone into the builder.
    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error>;

    /// Seal the builder to stop further writes and return the Sealed type.
    fn seal(self) -> Result<Self::Sealed, Error>;
}

///////////////////////////////////////////// SstEntry /////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
#[allow(clippy::enum_variant_names)]
enum SstEntry<'a> {
    #[prototk(10, bytes)]
    PlainBlock(&'a [u8]),
    // #[prototk(11, bytes)]
    // ZstdBlock(&'a [u8]),
    #[prototk(13, bytes)]
    FilterBlock(&'a [u8]),
    #[prototk(12, bytes)]
    FinalBlock(&'a [u8]),
}

impl SstEntry<'_> {
    fn bytes(&self) -> &[u8] {
        match self {
            SstEntry::PlainBlock(x) => x,
            SstEntry::FilterBlock(x) => x,
            SstEntry::FinalBlock(x) => x,
        }
    }

    fn crc32c(&self) -> u32 {
        crc32c::crc32c(self.bytes())
    }
}

impl Default for SstEntry<'_> {
    fn default() -> Self {
        Self::PlainBlock(&[])
    }
}

/////////////////////////////////////////// BlockMetadata //////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct BlockMetadata {
    #[prototk(13, uint64)]
    start: u64,
    #[prototk(14, uint64)]
    limit: u64,
    #[prototk(15, fixed32)]
    crc32c: u32,
    // NOTE(rescrv): If adding a field, update the constant for max size.
}

const BLOCK_METADATA_MAX_SZ: usize = 27;

impl BlockMetadata {
    fn sanity_check(&self) -> Result<(), Error> {
        if self.start >= self.limit {
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "block_metadata.start >= block_metadata.limit".to_string(),
            }
            .with_info("self.start", self.start)
            .with_info("self.limit", self.limit);
            return Err(err);
        }
        Ok(())
    }
}

//////////////////////////////////////////// FinalBlock ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct FinalBlock {
    #[prototk(16, message)]
    index_block: BlockMetadata,
    #[prototk(17, message)]
    filter_block: BlockMetadata,
    #[prototk(19, bytes32)]
    setsum: [u8; 32],
    #[prototk(20, uint64)]
    smallest_timestamp: u64,
    #[prototk(21, uint64)]
    biggest_timestamp: u64,
    // NOTE(rescrv): If adding a field, update the constant for max size.
    // This must be the final field of the struct.
    #[prototk(18, fixed64)]
    final_block_offset: u64,
}

#[rustfmt::skip]
const FINAL_BLOCK_MAX_SZ: usize = 2 + 10 + BLOCK_METADATA_MAX_SZ // index block
                                + 2 + 10 + BLOCK_METADATA_MAX_SZ // filter block
                                + 2 + 1 + setsum::SETSUM_BYTES // setsum
                                + 2 + 10 // smallest timestamp
                                + 2 + 10 // biggest timestamp
                                + 2 + 8; // final_block_offset;

//////////////////////////////////////////// SstMetadata ///////////////////////////////////////////

/// Metadata about an Sst.
#[derive(Clone, Eq, Message, Ord, PartialEq, PartialOrd)]
pub struct SstMetadata {
    /// The digest of the setsum covering this Sst.
    #[prototk(1, bytes32)]
    pub setsum: [u8; 32],
    /// The smallest key in the sst.
    #[prototk(2, bytes)]
    pub first_key: Vec<u8>,
    /// The largest key in the sst.
    #[prototk(3, bytes)]
    pub last_key: Vec<u8>,
    /// The smallest timestamp (not necessarily correlated with first_key).
    #[prototk(4, uint64)]
    pub smallest_timestamp: u64,
    /// The biggest timestamp (not necessarily correlated with last_key).
    #[prototk(5, uint64)]
    pub biggest_timestamp: u64,
    /// The file size.
    #[prototk(6, uint64)]
    pub file_size: u64,
}

impl SstMetadata {
    /// The first key, escaped for printing.
    pub fn first_key_escaped(&self) -> String {
        // TODO(rescrv): dedupe this with sst-dump
        String::from_utf8(
            self.first_key
                .iter()
                .flat_map(|b| std::ascii::escape_default(*b))
                .collect::<Vec<u8>>(),
        )
        .unwrap()
    }

    /// The last key, escaped for printing.
    pub fn last_key_escaped(&self) -> String {
        String::from_utf8(
            self.last_key
                .iter()
                .flat_map(|b| std::ascii::escape_default(*b))
                .collect::<Vec<u8>>(),
        )
        .unwrap()
    }
}

impl Default for SstMetadata {
    fn default() -> Self {
        let last_key = vec![0xffu8; MAX_KEY_LEN];
        Self {
            setsum: [0u8; 32],
            first_key: Vec::new(),
            last_key,
            smallest_timestamp: 0,
            biggest_timestamp: u64::MAX,
            file_size: 0,
        }
    }
}

impl Debug for SstMetadata {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "SstMetadata {{ setsum: {}, first_key: \"{}\", last_key: \"{}\", smallest_timestamp: {} biggest_timestamp: {}, file_size: {} }}",
            Setsum::from_digest(self.setsum).hexdigest(), self.first_key_escaped(), self.last_key_escaped(), self.smallest_timestamp, self.biggest_timestamp, self.file_size)
    }
}

impl From<SstMetadata> for indicio::Value {
    fn from(metadata: SstMetadata) -> indicio::Value {
        indicio::value!({
            setsum: Setsum::from_digest(metadata.setsum).hexdigest(),
            first_key: metadata.first_key_escaped(),
            last_key: metadata.last_key_escaped(),
            smallest_timestamp: metadata.smallest_timestamp,
            biggest_timestamp: metadata.biggest_timestamp,
            file_size: metadata.file_size,
        })
    }
}

//////////////////////////////////////////////// Sst ///////////////////////////////////////////////

/// An Sst represents an immutable sorted string table.
#[derive(Clone, Debug)]
pub struct Sst<W: Clone + Seek + Write + FileExt = FileHandle> {
    // The file backing the table.
    handle: W,
    // The final block of the table.
    final_block: FinalBlock,
    // Sst metadata.
    index_block: Block,
    // Bloom filter.
    filter: Filter,
    // Cache for metadata call.
    file_size: u64,
}

impl<W: Clone + Seek + Write + FileExt> Sst<W> {
    /// Open the provided path using options.
    pub fn new<P: AsRef<Path>>(_options: SstOptions, path: P) -> Result<Sst<FileHandle>, Error> {
        let handle = open_without_manager(path.as_ref())?;
        Sst::<FileHandle>::from_file_handle(handle)
    }

    /// Create an Sst from a file handle.
    pub fn from_file_handle(mut handle: W) -> Result<Self, Error> {
        SST_OPEN.click();
        // Read and parse the final block's offset
        let file_size = handle.seek(SeekFrom::End(0))?;
        if file_size < 8 {
            CORRUPTION.click();
            let err = Err(Error::Corruption {
                core: ErrorCore::default(),
                context: "file has fewer than eight bytes".to_string(),
            });
            return err;
        }
        let position = file_size - 8;
        let mut buf: Vec<u8> = vec![0, 0, 0, 0, 0, 0, 0, 0];
        handle.read_exact_at(&mut buf, position)?;
        let mut up = Unpacker::new(&buf);
        let final_block_offset: u64 = up.unpack().map_err(|e: buffertk::Error| {
            CORRUPTION.click();
            Error::UnpackError {
                core: ErrorCore::default(),
                error: e.into(),
                context: "parsing final block offset".to_string(),
            }
        })?;
        // Read and parse the final block
        if file_size < final_block_offset {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "final block offset is larger than file size".to_string(),
            }
            .with_info("final_block_offset", final_block_offset)
            .with_info("file_size", file_size);
            return Err(err);
        }
        let size_of_final_block = position + 8 - (final_block_offset);
        buf.resize(size_of_final_block as usize, 0);
        handle.read_exact_at(&mut buf, final_block_offset)?;
        let mut up = Unpacker::new(&buf);
        let final_block: FinalBlock = up.unpack().map_err(|e| {
            CORRUPTION.click();
            Error::UnpackError {
                core: ErrorCore::default(),
                error: e,
                context: "parsing final block".to_string(),
            }
        })?;
        final_block.index_block.sanity_check()?;
        final_block.filter_block.sanity_check()?;
        // Check that the final block's metadata is sane.
        if final_block.index_block.limit > final_block.filter_block.start {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "index_block runs past filter_block.start".to_string(),
            }
            .with_info("filter_block_start", final_block.filter_block.start)
            .with_info("index_block_limit", final_block.index_block.limit);
            return Err(err);
        }
        // Check that the final block's metadata is sane.
        if final_block.filter_block.limit > final_block_offset {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "filter_block runs past final_block_offset".to_string(),
            }
            .with_info("final_block_offset", final_block_offset)
            .with_info("filter_block_limit", final_block.filter_block.limit);
            return Err(err);
        }
        let index_block = Sst::load_block(&handle, &final_block.index_block)?;
        let filter = Sst::load_filter_block(&handle, &final_block.filter_block)?;
        Ok(Self {
            handle,
            final_block,
            index_block,
            filter,
            file_size,
        })
    }

    /// Approximate size of the sst's memory footprint.
    pub fn approximate_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.index_block.approximate_size()
            + self.filter.approximate_size()
    }

    /// Get a new cursor for the Sst.
    pub fn cursor(&self) -> SstCursor<W> {
        SST_CURSOR_NEW.click();
        SstCursor::<W>::new(self.clone())
    }

    /// Get the Sst's metadata.  This will involve reading the first and last keys from disk.
    pub fn metadata(&self) -> Result<SstMetadata, Error> {
        SST_METADATA.click();
        let mut cursor = self.cursor();
        // First key.
        cursor.seek_to_first()?;
        cursor.next()?;
        let kr = cursor.key();
        let first_key = match kr {
            Some(kr) => Vec::from(kr.key),
            None => Vec::new(),
        };
        // Last key.
        cursor.seek_to_last()?;
        cursor.prev()?;
        let kr = cursor.key();
        let last_key = match kr {
            Some(kr) => Vec::from(kr.key),
            None => MAX_KEY.to_vec(),
        };
        // Metadata
        Ok(SstMetadata {
            setsum: self.final_block.setsum,
            first_key,
            last_key,
            smallest_timestamp: self.final_block.smallest_timestamp,
            biggest_timestamp: self.final_block.biggest_timestamp,
            file_size: self.file_size,
        })
    }

    /// Return the setsum stored in the final block of the sst.
    pub fn fast_setsum(&self) -> Setsum {
        Setsum::from_digest(self.final_block.setsum)
    }

    /// Inspect the sst by printing its internal structure.
    pub fn inspect(&self) -> Result<(), Error> {
        let mut meta_cursor = self.index_block.cursor();
        meta_cursor.seek_to_first()?;
        meta_cursor.next()?;
        while let Some(kvr) = meta_cursor.key_value() {
            let metadata = SstCursor::<W>::metadata_from_kvr(kvr)
                .expect("metadata should parse")
                .unwrap();
            println!("{metadata:?}");
            let block = Self::load_block(&self.handle, &metadata)?;
            let mut block_cursor = block.cursor();
            block_cursor.seek_to_first()?;
            block_cursor.next()?;
            while let Some(kvr) = block_cursor.key_value() {
                println!(
                    "[{}..{}] {:?}",
                    block_cursor.offset(),
                    block_cursor.next_offset(),
                    kvr
                );
                block_cursor.next()?;
            }
            meta_cursor.next()?;
        }
        Ok(())
    }

    fn load_block(file: &W, block_metadata: &BlockMetadata) -> Result<Block, Error> {
        SST_LOAD_BLOCK.click();
        block_metadata.sanity_check()?;
        let amt = (block_metadata.limit - block_metadata.start) as usize;
        let mut buf: Vec<u8> = vec![0u8; amt];
        file.read_exact_at(&mut buf, block_metadata.start)?;
        let mut up = Unpacker::new(&buf);
        let table_entry: SstEntry = up.unpack().map_err(|e| {
            CORRUPTION.click();
            Error::UnpackError {
                core: ErrorCore::default(),
                error: e,
                context: "parsing table entry".to_string(),
            }
        })?;
        if table_entry.crc32c() != block_metadata.crc32c {
            CORRUPTION.click();
            let err = Error::Crc32cFailure {
                core: ErrorCore::default(),
                start: block_metadata.start,
                limit: block_metadata.limit,
                crc32c: block_metadata.crc32c,
            };
            return Err(err);
        }
        match table_entry {
            SstEntry::PlainBlock(bytes) => Block::new(bytes.into()),
            SstEntry::FilterBlock(_) => {
                CORRUPTION.click();
                Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "tried loading filter block".to_string(),
                })
            }
            SstEntry::FinalBlock(_) => {
                CORRUPTION.click();
                Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "tried loading final block".to_string(),
                })
            }
        }
    }

    fn load_filter_block(file: &W, block_metadata: &BlockMetadata) -> Result<Filter, Error> {
        SST_LOAD_FILTER.click();
        block_metadata.sanity_check()?;
        let amt = (block_metadata.limit - block_metadata.start) as usize;
        let mut buf: Vec<u8> = vec![0u8; amt];
        file.read_exact_at(&mut buf, block_metadata.start)?;
        let mut up = Unpacker::new(&buf);
        let table_entry: SstEntry = up.unpack().map_err(|e| {
            CORRUPTION.click();
            Error::UnpackError {
                core: ErrorCore::default(),
                error: e,
                context: "parsing table entry".to_string(),
            }
        })?;
        if table_entry.crc32c() != block_metadata.crc32c {
            CORRUPTION.click();
            let err = Error::Crc32cFailure {
                core: ErrorCore::default(),
                start: block_metadata.start,
                limit: block_metadata.limit,
                crc32c: block_metadata.crc32c,
            };
            return Err(err);
        }
        match table_entry {
            SstEntry::FilterBlock(bytes) => Filter::try_from(bytes).map_err(|err| {
                CORRUPTION.click();
                Error::Corruption {
                    core: ErrorCore::default(),
                    context: format!("bad filter block: {err}"),
                }
            }),
            SstEntry::PlainBlock(_) => {
                CORRUPTION.click();
                Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "tried loading plain block".to_string(),
                })
            }
            SstEntry::FinalBlock(_) => {
                CORRUPTION.click();
                Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "tried loading final block".to_string(),
                })
            }
        }
    }

    pub fn load(
        &self,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Error> {
        *is_tombstone = false;
        if !self.filter.check(key) {
            SST_BLOOM_NEGATIVE.click();
            return Ok(None);
        }
        let mut cursor = self.cursor();
        cursor.seek(key)?;
        let target = KeyRef { key, timestamp };
        while let Some(kr) = cursor.key() {
            if kr >= target {
                break;
            } else {
                cursor.next()?;
            }
        }
        if let Some(kvr) = cursor.key_value() {
            if kvr.key == key {
                *is_tombstone = kvr.value.is_none();
                Ok(kvr.value.as_ref().map(|v| v.to_vec()))
            } else {
                SST_BLOOM_FALSE_POSITIVE.click();
                Ok(None)
            }
        } else {
            SST_BLOOM_FALSE_POSITIVE.click();
            Ok(None)
        }
    }

    pub fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<BoundsCursor<PruningCursor<SstCursor<W>>>, Error> {
        let pruning = PruningCursor::new(self.cursor(), timestamp)?;
        BoundsCursor::new(pruning, start_bound, end_bound)
    }
}

///////////////////////////////////////// BlockCompression /////////////////////////////////////////

/// An enum matching the types of compression supported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockCompression {
    /// Do not use any compression (default).
    NoCompression,
}

impl BlockCompression {
    fn compress<'a>(&self, bytes: &'a [u8], scratch: &'a mut Vec<u8>) -> SstEntry<'a> {
        match self {
            BlockCompression::NoCompression => {
                scratch.clear();
                SstEntry::PlainBlock(bytes)
            }
        }
    }
}

//////////////////////////////////////////// SstOptions ////////////////////////////////////////////

/// The minimum target block size.
pub const CLAMP_MIN_TARGET_BLOCK_SIZE: u32 = 1u32 << 12;
/// The maximum target block size.
pub const CLAMP_MAX_TARGET_BLOCK_SIZE: u32 = 1u32 << 24;

/// The minimum target file size.
pub const CLAMP_MIN_TARGET_FILE_SIZE: u32 = 1u32 << 12;
/// The maximum target file size.
pub const CLAMP_MAX_TARGET_FILE_SIZE: u32 = TABLE_FULL_SIZE as u32;

/// The minimum minimum file size.
pub const CLAMP_MIN_MINIMUM_FILE_SIZE: u32 = 1u32 << 12;
/// The maximum minimum file size.
pub const CLAMP_MAX_MINIMUM_FILE_SIZE: u32 = TABLE_FULL_SIZE as u32;

/// Options for working with Ssts.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct SstOptions {
    /// Options for the blocks of the sst.
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    block: BlockBuilderOptions,
    /// The type of compression in use.
    // TODO(rescrv): arrrg needs an enum helper.
    block_compression: BlockCompression,
    /// The target block size.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Target block size.", "BYTES")
    )]
    target_block_size: usize,
    /// The target file size.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Target file size.", "BYTES")
    )]
    target_file_size: usize,
    /// The minimum file size.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Minimum file size.", "BYTES")
    )]
    minimum_file_size: usize,
    /// The write buffer size.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Write buffer size.", "BYTES")
    )]
    write_buffer_size: usize,
    /// The number of bits to allocate per key-value pair in bloom filters.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Bloom filter bits per key.", "BITS/KEY")
    )]
    bloom_filter_bits: u8,
}

impl SstOptions {
    /// Set the block options.
    pub fn block(mut self, block: BlockBuilderOptions) -> Self {
        self.block = block;
        self
    }

    /// Set the block compression.
    pub fn block_compression(mut self, block_compression: BlockCompression) -> Self {
        self.block_compression = block_compression;
        self
    }

    /// Set the target block size.
    pub fn target_block_size(mut self, target_block_size: u32) -> Self {
        self.target_block_size = target_block_size
            .clamp(CLAMP_MIN_TARGET_BLOCK_SIZE, CLAMP_MAX_TARGET_BLOCK_SIZE)
            as usize;
        self
    }

    /// Set the target file size.
    pub fn target_file_size(mut self, target_file_size: u32) -> Self {
        self.target_file_size =
            target_file_size.clamp(CLAMP_MIN_TARGET_FILE_SIZE, CLAMP_MAX_TARGET_FILE_SIZE) as usize;
        self
    }

    /// Set the minimum file size
    pub fn minimum_file_size(mut self, minimum_file_size: u32) -> Self {
        self.minimum_file_size = minimum_file_size
            .clamp(CLAMP_MIN_MINIMUM_FILE_SIZE, CLAMP_MAX_MINIMUM_FILE_SIZE)
            as usize;
        self
    }
}

impl Default for SstOptions {
    fn default() -> SstOptions {
        SstOptions {
            block: BlockBuilderOptions::default(),
            block_compression: BlockCompression::NoCompression,
            target_block_size: 4096,
            target_file_size: 1 << 26,
            minimum_file_size: 1 << 22,
            write_buffer_size: 1 << 22,
            bloom_filter_bits: 17,
        }
    }
}

//////////////////////////////////////////// SstBuilder ////////////////////////////////////////////

/// Build Ssts by providing keys in-order.
pub struct SstBuilder {
    // Options for every "normal" table entry.
    options: SstOptions,
    // The most recent that was successfully written.  Update only after writing to the block to
    // which a key is written.
    last_key: Vec<u8>,
    last_timestamp: u64,
    // The currently building table entry.
    block_builder: Option<BlockBuilder>,
    block_start_offset: usize,
    // The index that trails the file.  Written on seal.
    bytes_written: usize,
    index_block: BlockBuilder,
    // The entries to insert into the bloom filter covering the keys in this file.
    filter: Vec<u64>,
    // The checksum of the file.
    setsum: Setsum,
    // Timestamps seen.
    smallest_timestamp: u64,
    biggest_timestamp: u64,
    // Output information.
    output: BufWriter<File>,
    path: PathBuf,
}

impl SstBuilder {
    /// Create a new SstBuilder.
    pub fn new<P: AsRef<Path>>(options: SstOptions, path: P) -> Result<Self, Error> {
        BUILDER_NEW.click();
        let block_options = options.block.clone();
        let write_buffer_size = options.write_buffer_size;
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.as_ref())
            .as_z()
            .with_info("open", path.as_ref().to_string_lossy())?;
        Ok(SstBuilder {
            options,
            last_key: Vec::new(),
            last_timestamp: u64::MAX,
            block_builder: None,
            block_start_offset: 0,
            bytes_written: 0,
            index_block: BlockBuilder::new(block_options),
            filter: vec![],
            setsum: Setsum::default(),
            smallest_timestamp: u64::MAX,
            biggest_timestamp: 0,
            output: BufWriter::with_capacity(write_buffer_size, output),
            path: path.as_ref().to_path_buf(),
        })
    }

    fn enforce_sort_order(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        BUILDER_COMPARE_KEY.click();
        if KeyRef::new(&self.last_key, self.last_timestamp).cmp(&KeyRef::new(key, timestamp))
            != Ordering::Less
        {
            Err(Error::SortOrder {
                core: ErrorCore::default(),
                last_key: self.last_key.clone(),
                last_timestamp: self.last_timestamp,
                new_key: key.to_vec(),
                new_timestamp: timestamp,
            })
        } else {
            Ok(())
        }
    }

    fn assign_last_key(&mut self, key: &[u8], timestamp: u64) {
        BUILDER_ASSIGN_LAST_KEY.click();
        self.last_key.truncate(0);
        self.last_key.extend_from_slice(key);
        self.last_timestamp = timestamp;
        if self.smallest_timestamp > timestamp {
            self.smallest_timestamp = timestamp;
        }
        if self.biggest_timestamp < timestamp {
            self.biggest_timestamp = timestamp;
        }
    }

    fn start_new_block(&mut self) -> Result<(), Error> {
        BUILDER_START_NEW_BLOCK.click();
        if self.block_builder.is_some() {
            LOGIC_ERROR.click();
            return Err(Error::LogicError {
                core: ErrorCore::default(),
                context: "called start_new_block() when block_builder is not None".to_string(),
            });
        }
        self.block_builder = Some(BlockBuilder::new(self.options.block.clone()));
        self.block_start_offset = self.bytes_written;
        Ok(())
    }

    fn flush_block(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        BUILDER_FLUSH_BLOCK.click();
        if self.block_builder.is_none() {
            LOGIC_ERROR.click();
            return Err(Error::LogicError {
                core: ErrorCore::default(),
                context: "self.block_builder.is_none()".to_string(),
            });
        }
        // Metadata for the block.
        let start = self.bytes_written as u64;
        // Write out the block.
        let block = self.block_builder.take().unwrap().seal()?;
        let bytes = block.as_bytes();
        let mut scratch = Vec::new();
        let entry = self.options.block_compression.compress(bytes, &mut scratch);
        let crc32c = entry.crc32c();
        let pa = stack_pack(entry);
        self.bytes_written += pa.stream(&mut self.output).as_z()?;
        // Prepare the block metadata.
        let limit = self.bytes_written as u64;
        let block_metadata = BlockMetadata {
            start,
            limit,
            crc32c,
        };
        block_metadata.sanity_check()?;
        let value = stack_pack(block_metadata).to_vec();
        // Find a dividing key that falls between last_{key,timestamp} and {key,timestamp}.  In
        // this way, a seek to a {key,timestamp} will fall before this dividing key and point to
        // the block.  This way we seek in the index block and get appropriate BlockMetadata.
        let (dividing_key, dividing_timestamp) =
            divide_keys(&self.last_key, self.last_timestamp, key, timestamp);
        self.index_block
            .put(&dividing_key, dividing_timestamp, &value)
    }

    fn get_block(&mut self, key: &[u8], timestamp: u64) -> Result<&mut BlockBuilder, Error> {
        if self.block_builder.is_none() {
            self.start_new_block()?;
        } else {
            let block_builder: &mut BlockBuilder = self.block_builder.as_mut().unwrap();
            if block_builder.approximate_size() > self.options.target_block_size {
                self.flush_block(key, timestamp)?;
                self.start_new_block()?;
            }
        }

        Ok(self.block_builder.as_mut().unwrap())
    }
}

impl Builder for SstBuilder {
    type Sealed = Sst;

    fn approximate_size(&self) -> usize {
        BUILDER_APPROX_SIZE.click();
        let mut sum = self.bytes_written;
        sum += match &self.block_builder {
            Some(block) => block.approximate_size(),
            None => 0,
        };
        sum += 1 + self.index_block.approximate_size();
        sum += FINAL_BLOCK_MAX_SZ;
        sum
    }

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        BUILDER_PUT.click();
        check_key_len(key)?;
        check_value_len(value)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.put(key, timestamp, value)?;
        self.filter.push(Filter::defer_insert(key));
        self.setsum.put(key, timestamp, value);
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        BUILDER_DEL.click();
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.del(key, timestamp)?;
        self.filter.push(Filter::defer_insert(key));
        self.setsum.del(key, timestamp);
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    fn seal(self) -> Result<Sst, Error> {
        BUILDER_SEAL.click();
        let mut builder = self;
        // Flush the block we have.
        if builder.block_builder.is_some() {
            let (key, timestamp) = minimal_successor_key(&builder.last_key, builder.last_timestamp);
            builder.flush_block(&key, timestamp)?;
        }
        fn flush_block(builder: &mut SstBuilder, entry: SstEntry) -> Result<BlockMetadata, Error> {
            let start = builder.bytes_written as u64;
            let crc32c = entry.crc32c();
            let pa = stack_pack(entry);
            builder.bytes_written += pa.stream(&mut builder.output).as_z()?;
            let limit = builder.bytes_written as u64;
            Ok(BlockMetadata {
                start,
                limit,
                crc32c,
            })
        }
        // Flush the index block after the data blocks.
        let index_block = builder.index_block.clone().seal()?;
        let index_bytes = index_block.as_bytes();
        let index_block = flush_block(&mut builder, SstEntry::PlainBlock(index_bytes))?;
        // Flush the filter block after the index block.
        let mut filter = Filter::new(
            (builder.filter.len() as u32).saturating_mul(builder.options.bloom_filter_bits as u32),
        );
        for x in builder.filter.iter() {
            filter.deferred_insert(*x);
        }
        let filter_bytes = filter.to_bytes();
        let filter_block = flush_block(&mut builder, SstEntry::FilterBlock(&filter_bytes))?;
        // Update timestamps if nothing written
        if builder.smallest_timestamp > builder.biggest_timestamp {
            builder.smallest_timestamp = 0;
            builder.biggest_timestamp = 0;
        }
        // Our final_block
        let final_block = FinalBlock {
            index_block,
            filter_block,
            final_block_offset: builder.bytes_written as u64,
            setsum: builder.setsum.digest(),
            smallest_timestamp: builder.smallest_timestamp,
            biggest_timestamp: builder.biggest_timestamp,
        };
        let pa = stack_pack(final_block);
        builder.bytes_written += pa.stream(&mut builder.output).as_z()?;
        // fsync
        builder.output.flush().as_z()?;
        builder.output.get_mut().sync_all().as_z()?;
        Sst::<FileHandle>::new(builder.options, builder.path)
    }
}

////////////////////////////////////////// SstMultiBuilder /////////////////////////////////////////

/// Create an SstBuilder that will create numbered files of similar prefix and suffix.
pub struct SstMultiBuilder {
    prefix: PathBuf,
    suffix: String,
    counter: u64,
    options: SstOptions,
    builder: Option<SstBuilder>,
    paths: Vec<PathBuf>,
}

impl SstMultiBuilder {
    /// Create Ssts with prefix, suffix, and options.
    pub fn new(prefix: PathBuf, suffix: String, options: SstOptions) -> Self {
        Self {
            prefix,
            suffix,
            counter: 0,
            options,
            builder: None,
            paths: Vec::new(),
        }
    }

    /// Provide a hint that this would be a good spot to split to create a new sst.
    pub fn split_hint(&mut self) -> Result<(), Error> {
        if self.builder.is_some() {
            let size = self.builder.as_mut().unwrap().approximate_size();
            if size >= TABLE_FULL_SIZE || size >= self.options.minimum_file_size {
                let builder = self.builder.take().unwrap();
                builder.seal()?;
            }
        }
        Ok(())
    }

    fn get_builder(&mut self) -> Result<&mut SstBuilder, Error> {
        if self.builder.is_some() {
            let size = self.builder.as_mut().unwrap().approximate_size();
            if size >= TABLE_FULL_SIZE || size >= self.options.target_file_size {
                let builder = self.builder.take().unwrap();
                builder.seal()?;
                return self.get_builder();
            }
            return Ok(self.builder.as_mut().unwrap());
        }
        let path = self
            .prefix
            .join(PathBuf::from(format!("{}{}", self.counter, self.suffix)));
        self.paths.push(path.clone());
        self.counter += 1;
        self.builder = Some(SstBuilder::new(self.options.clone(), path)?);
        Ok(self.builder.as_mut().unwrap())
    }
}

impl Builder for SstMultiBuilder {
    type Sealed = Vec<PathBuf>;

    fn approximate_size(&self) -> usize {
        match &self.builder {
            Some(b) => b.approximate_size(),
            None => 0,
        }
    }

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        self.get_builder()?.put(key, timestamp, value)
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        self.get_builder()?.del(key, timestamp)
    }

    fn seal(mut self) -> Result<Vec<PathBuf>, Error> {
        let builder = match self.builder.take() {
            Some(b) => b,
            None => {
                return Ok(self.paths);
            }
        };
        builder.seal()?;
        Ok(self.paths)
    }
}

///////////////////////////////////////////// SstCursor ////////////////////////////////////////////

/// A cursor over an Sst.
#[derive(Clone, Debug)]
pub struct SstCursor<W: Clone + Seek + Write + FileExt = FileHandle> {
    table: Sst<W>,
    // The position in the table.  When meta_cursor is at its extremes, block_cursor is None.
    // Otherwise, block_cursor is positioned at the block referred to by the most recent
    // KVP-returning call to meta_cursor.
    meta_cursor: BlockCursor,
    block_cursor: Option<BlockCursor>,
}

impl<W: Clone + Seek + Write + FileExt> SstCursor<W> {
    fn new(table: Sst<W>) -> Self {
        let meta_cursor = table.index_block.cursor();
        Self {
            table,
            meta_cursor,
            block_cursor: None,
        }
    }

    fn meta_prev(&mut self) -> Result<Option<BlockMetadata>, Error> {
        SST_CURSOR_META_PREV.click();
        self.meta_cursor.prev()?;
        let kvr = match self.meta_cursor.key_value() {
            Some(kvr) => kvr,
            None => {
                self.seek_to_first()?;
                return Ok(None);
            }
        };
        SstCursor::<W>::metadata_from_kvr(kvr)
    }

    fn meta_next(&mut self) -> Result<Option<BlockMetadata>, Error> {
        SST_CURSOR_META_NEXT.click();
        self.meta_cursor.next()?;
        let kvr = match self.meta_cursor.key_value() {
            Some(kvr) => kvr,
            None => {
                self.seek_to_last()?;
                return Ok(None);
            }
        };
        SstCursor::<W>::metadata_from_kvr(kvr)
    }

    fn meta_value(&mut self) -> Result<Option<BlockMetadata>, Error> {
        let kvr = match self.meta_cursor.key_value() {
            Some(kvr) => kvr,
            None => {
                return Ok(None);
            }
        };
        SstCursor::<W>::metadata_from_kvr(kvr)
    }

    fn metadata_from_kvr(kvr: KeyValueRef) -> Result<Option<BlockMetadata>, Error> {
        let value = match kvr.value {
            Some(v) => v,
            None => {
                CORRUPTION.click();
                return Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "meta block has null value".to_string(),
                });
            }
        };
        let mut up = Unpacker::new(value);
        let metadata: BlockMetadata = up.unpack().map_err(|e| {
            CORRUPTION.click();
            Error::UnpackError {
                core: ErrorCore::default(),
                error: e,
                context: "parsing block metadata".to_string(),
            }
        })?;
        Ok(Some(metadata))
    }
}

impl<W: Clone + Seek + Write + FileExt> Cursor for SstCursor<W> {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        SST_CURSOR_SEEK_TO_FIRST.click();
        self.meta_cursor.seek_to_first()?;
        self.block_cursor = None;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        SST_CURSOR_SEEK_TO_LAST.click();
        self.meta_cursor.seek_to_last()?;
        self.block_cursor = None;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        SST_CURSOR_SEEK.click();
        self.meta_cursor.seek(key)?;
        let metadata = match self.meta_value()? {
            Some(m) => m,
            None => {
                return self.seek_to_last();
            }
        };
        let block = Sst::<W>::load_block(&self.table.handle, &metadata)?;
        let mut block_cursor = block.cursor();
        block_cursor.seek(key)?;
        if block_cursor.key().is_none() {
            let metadata = match self.meta_next()? {
                Some(m) => m,
                None => {
                    return self.seek_to_last();
                }
            };
            let block = Sst::<W>::load_block(&self.table.handle, &metadata)?;
            let mut block_cursor = block.cursor();
            block_cursor.seek(key)?;
            self.block_cursor = Some(block_cursor);
        } else {
            self.block_cursor = Some(block_cursor);
        }
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        SST_CURSOR_PREV.click();
        if self.block_cursor.is_none() {
            let metadata = match self.meta_prev()? {
                Some(m) => m,
                None => {
                    return self.seek_to_first();
                }
            };
            let block = Sst::<W>::load_block(&self.table.handle, &metadata)?;
            let mut block_cursor = block.cursor();
            block_cursor.seek_to_last()?;
            self.block_cursor = Some(block_cursor);
        }
        assert!(self.block_cursor.is_some());
        let block_cursor: &mut BlockCursor = self.block_cursor.as_mut().unwrap();
        block_cursor.prev()?;
        match block_cursor.key_value() {
            Some(_) => Ok(()),
            None => {
                self.block_cursor = None;
                self.prev()
            }
        }
    }

    fn next(&mut self) -> Result<(), Error> {
        SST_CURSOR_NEXT.click();
        if self.block_cursor.is_none() {
            let metadata = match self.meta_next()? {
                Some(m) => m,
                None => {
                    return self.seek_to_last();
                }
            };
            let block = Sst::<W>::load_block(&self.table.handle, &metadata)?;
            let mut block_cursor = block.cursor();
            block_cursor.seek_to_first()?;
            self.block_cursor = Some(block_cursor);
        }
        assert!(self.block_cursor.is_some());
        let block_cursor: &mut BlockCursor = self.block_cursor.as_mut().unwrap();
        block_cursor.next()?;
        match block_cursor.key_value() {
            Some(_) => Ok(()),
            None => {
                self.block_cursor = None;
                self.next()
            }
        }
    }

    fn key(&self) -> Option<KeyRef> {
        match &self.block_cursor {
            Some(cursor) => cursor.key(),
            None => None,
        }
    }

    fn value(&self) -> Option<&[u8]> {
        match &self.block_cursor {
            Some(cursor) => cursor.value(),
            None => None,
        }
    }
}

impl From<Sst> for SstCursor {
    fn from(table: Sst) -> Self {
        SST_CURSOR_NEW.click();
        Self::new(table)
    }
}

//////////////////////////////////////////// divide_keys ///////////////////////////////////////////

/// Return a key that is >= lhs and < rhs.
fn divide_keys(
    key_lhs: &[u8],
    timestamp_lhs: u64,
    key_rhs: &[u8],
    timestamp_rhs: u64,
) -> (Vec<u8>, u64) {
    assert!(
        KeyRef::new(key_lhs, timestamp_lhs).cmp(&KeyRef::new(key_rhs, timestamp_rhs))
            == Ordering::Less
    );
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
        d_key.extend_from_slice(&key_lhs[0..shared + 1]);
        d_key[shared] = key_lhs[shared] + 1;
        d_timestamp = 0;
    } else {
        d_key.extend_from_slice(key_lhs);
        d_timestamp = timestamp_lhs;
    }
    let cmp_lhs = KeyRef::new(key_lhs, timestamp_lhs).cmp(&KeyRef::new(&d_key, d_timestamp));
    let cmp_rhs = KeyRef::new(&d_key, d_timestamp).cmp(&KeyRef::new(key_rhs, timestamp_rhs));
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
    fn u64_is_usize() {
        assert_eq!(u64::BITS, usize::BITS);
    }

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
            for (i, b) in buf.iter_mut().enumerate() {
                *b = i as u8;
            }
            assert_eq!(0x46dd794e, crc32c::crc32c(&buf));

            for (i, b) in buf.iter_mut().enumerate() {
                *b = 31 - i as u8;
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
