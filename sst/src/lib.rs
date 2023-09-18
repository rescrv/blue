#[macro_use]
extern crate arrrg_derive;

extern crate prototk;
#[macro_use]
extern crate prototk_derive;

use std::cmp;
use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter, Write};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use buffertk::{stack_pack, Buffer, Packable, Unpacker};

use biometrics::Counter;

use tatl::{HeyListen, Stationary};

use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

pub mod block;
pub mod file_manager;
pub mod merging_cursor;
pub mod pruning_cursor;
pub mod reference;
pub mod sequence_cursor;
pub mod setsum;

use block::{Block, BlockCursor, BlockBuilder, BlockBuilderOptions};
use file_manager::{open_without_manager, FileHandle};
use crate::setsum::Setsum;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOGIC_ERROR: Counter = Counter::new("sst.logic_error");
static LOGIC_ERROR_MONITOR: Stationary = Stationary::new("sst.logic_error", &LOGIC_ERROR);

static CORRUPTION: Counter = Counter::new("sst.corruption");
static CORRUPTION_MONITOR: Stationary = Stationary::new("sst.corruption", &CORRUPTION);

static KEY_TOO_LARGE: Counter = Counter::new("sst.error.key_too_large");
static KEY_TOO_LARGE_MONITOR: Stationary = Stationary::new("sst.error.key_too_large", &KEY_TOO_LARGE);

static VALUE_TOO_LARGE: Counter = Counter::new("sst.error.value_too_large");
static VALUE_TOO_LARGE_MONITOR: Stationary = Stationary::new("sst.error.value_too_large", &VALUE_TOO_LARGE);

static TABLE_FULL: Counter = Counter::new("sst.error.table_full");
static TABLE_FULL_MONITOR: Stationary = Stationary::new("sst.error.table_full", &TABLE_FULL);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOGIC_ERROR_MONITOR);
    hey_listen.register_stationary(&CORRUPTION_MONITOR);
    hey_listen.register_stationary(&KEY_TOO_LARGE_MONITOR);
    hey_listen.register_stationary(&VALUE_TOO_LARGE_MONITOR);
    hey_listen.register_stationary(&TABLE_FULL_MONITOR);

    file_manager::register_monitors(hey_listen);
}

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MAX_KEY_LEN: usize = 1usize << 14; /* 16KiB */
pub const MAX_VALUE_LEN: usize = 1usize << 15; /* 32KiB */

// NOTE(rescrv):  This is an approximate size.  This constant isn't intended to be a maximum size,
// but rather a size that, once exceeded, will cause the table to return a TableFull error.  The
// general pattern is that the block will exceed this size by up to one key-value pair, so subtract
// some slop.  64MiB is overkill, but will last for awhile.
pub const TABLE_FULL_SIZE: usize = (1usize << 30) - (1usize << 26); /* 1GiB - 64MiB */

fn check_key_len(key: &[u8]) -> Result<(), Error> {
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

fn check_value_len(value: &[u8]) -> Result<(), Error> {
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

fn check_table_size(size: usize) -> Result<(), Error> {
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

#[derive(Clone, Debug, Message)]
pub enum Error {
    #[prototk(442368, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(442369, message)]
    KeyTooLarge {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint64)]
        length: usize,
        #[prototk(3, uint64)]
        limit: usize,
    },
    #[prototk(442370, message)]
    ValueTooLarge {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint64)]
        length: usize,
        #[prototk(3, uint64)]
        limit: usize,
    },
    #[prototk(442371, message)]
    SortOrder {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, bytes)]
        last_key: Vec<u8>,
        #[prototk(3, uint64)]
        last_timestamp: u64,
        #[prototk(4, bytes)]
        new_key: Vec<u8>,
        #[prototk(5, uint64)]
        new_timestamp: u64,
    },
    #[prototk(442372, message)]
    TableFull {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint64)]
        size: usize,
        #[prototk(3, uint64)]
        limit: usize,
    },
    #[prototk(442373, message)]
    BlockTooSmall {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint64)]
        length: usize,
        #[prototk(3, uint64)]
        required: usize,
    },
    #[prototk(442374, message)]
    UnpackError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        error: prototk::Error,
        #[prototk(3, string)]
        context: String,
    },
    #[prototk(442375, message)]
    Crc32cFailure {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint64)]
        start: u64,
        #[prototk(3, uint64)]
        limit: u64,
        #[prototk(3, fixed32)]
        crc32c: u32,
    },
    #[prototk(442376, message)]
    Corruption {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        context: String,
    },
    #[prototk(442377, message)]
    LogicError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        context: String,
    },
    #[prototk(442378, message)]
    SystemError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(442379, message)]
    TooManyOpenFiles {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint64)]
        limit: usize,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::Success { core, .. } => { core },
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::Crc32cFailure { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::Success { core, .. } => { core },
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::Crc32cFailure { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
        }
    }
}

impl Default for Error {
    fn default() -> Self {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Error::Success { core: _ } => fmt
                .debug_struct("Success")
                .finish(),
            Error::KeyTooLarge { core: _, length, limit } => fmt
                .debug_struct("KeyTooLarge")
                .field("length", length)
                .field("limit", limit)
                .finish(),
            Error::ValueTooLarge { core: _, length, limit } => fmt
                .debug_struct("ValueTooLarge")
                .field("length", length)
                .field("limit", limit)
                .finish(),
            Error::SortOrder { core: _, last_key, last_timestamp, new_key, new_timestamp } => fmt
                .debug_struct("SortOrder")
                .field("last_key", last_key)
                .field("last_timestamp", last_timestamp)
                .field("new_key", new_key)
                .field("new_timestamp", new_timestamp)
                .finish(),
            Error::TableFull { core: _, size, limit } => fmt
                .debug_struct("TableFull")
                .field("size", size)
                .field("limit", limit)
                .finish(),
            Error::BlockTooSmall { core: _, length, required } => fmt
                .debug_struct("BlockTooSmall")
                .field("length", length)
                .field("required", required)
                .finish(),
            Error::UnpackError { core: _, error, context } => fmt
                .debug_struct("UnpackError")
                .field("error", error)
                .field("context", context)
                .finish(),
            Error::Crc32cFailure { core: _, start, limit, crc32c } => fmt
                .debug_struct("Crc32cFailure")
                .field("start", start)
                .field("limit", limit)
                .field("crc32c", crc32c)
                .finish(),
            Error::Corruption { core: _, context } => fmt
                .debug_struct("Corruption")
                .field("context", context)
                .finish(),
            Error::LogicError { core: _, context } => fmt
                .debug_struct("LogicError")
                .field("context", context)
                .finish(),
            Error::SystemError { core: _, what } => fmt
                .debug_struct("SystemError")
                .field("what", what)
                .finish(),
            Error::TooManyOpenFiles { core: _, limit } => fmt
                .debug_struct("TooManyOpenFiles")
                .field("limit", limit)
                .finish(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError {
            core: ErrorCore::default(),
            what: format!("{:?}", what),
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

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}", self) + "\n" + &self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.core_mut().set_token(identifier, value);
        self
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.core_mut().set_url(identifier, url);
        self
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error where X: Debug {
        self.core_mut().set_variable(variable, x);
        self
    }
}

iotoz!{Error}

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

impl<'a> From<KeyValueRef<'a>> for KeyValuePair {
    fn from(kvr: KeyValueRef<'a>) -> Self {
        Self {
            key: kvr.key.into(),
            timestamp: kvr.timestamp,
            value: kvr.value.map(|v| v.into()),
        }
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

impl<'a> Default for KeyValueEntry<'a> {
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

pub trait Builder {
    type Sealed;

    fn approximate_size(&self) -> usize;

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error>;
    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error>;

    fn seal(self) -> Result<Self::Sealed, Error>;
}

///////////////////////////////////////////// SstEntry /////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum SstEntry<'a> {
    #[prototk(10, bytes)]
    PlainBlock(&'a [u8]),
    // #[prototk(11, bytes)]
    // ZstdBlock(&'a [u8]),
    #[prototk(12, bytes)]
    FinalBlock(&'a [u8]),
}

impl<'a> SstEntry<'a> {
    fn bytes(&self) -> &[u8] {
        match self {
            SstEntry::PlainBlock(x) => x,
            SstEntry::FinalBlock(x) => x,
        }
    }

    fn crc32c(&self) -> u32 {
        crc32c::crc32c(self.bytes())
    }
}

impl<'a> Default for SstEntry<'a> {
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
            .with_variable("self.start", self.start)
            .with_variable("self.limit", self.limit);
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
    // #[prototk(17, message)]
    // filter_block: BlockMetadata,
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
const FINAL_BLOCK_MAX_SZ: usize = 2 + 10 + BLOCK_METADATA_MAX_SZ
                                + 2 + 1 + setsum::SETSUM_BYTES
                                + 2 + 10
                                + 2 + 10
                                + 2 + 8;

/////////////////////////////////////////// TableMetadata //////////////////////////////////////////

pub trait TableMetadata {
    fn first_key(&self) -> KeyRef;
    fn last_key(&self) -> KeyRef;
}

//////////////////////////////////////////// SstMetadata ///////////////////////////////////////////

#[derive(Clone, Eq, Message, Ord, PartialEq, PartialOrd)]
pub struct SstMetadata {
    #[prototk(1, bytes32)]
    pub setsum: [u8; 32],
    #[prototk(2, bytes)]
    pub first_key: Buffer,
    #[prototk(3, bytes)]
    pub last_key: Buffer,
    #[prototk(4, uint64)]
    pub smallest_timestamp: u64,
    #[prototk(5, uint64)]
    pub biggest_timestamp: u64,
    #[prototk(6, uint64)]
    pub file_size: u64,
}

impl SstMetadata {
    // TODO(rescrv): dedupe with the other implementations.
    pub fn setsum(&self) -> String {
        let mut setsum = String::with_capacity(68);
        for i in 0..self.setsum.len() {
            write!(&mut setsum, "{:02x}", self.setsum[i]).expect("unable to write to string");
        }
        setsum
    }

    pub fn first_key_escaped(&self) -> String {
        String::from_utf8(
            self.first_key
                .as_bytes()
                .iter()
                .flat_map(|b| std::ascii::escape_default(*b))
                .collect::<Vec<u8>>(),
        )
        .unwrap()
    }

    pub fn last_key_escaped(&self) -> String {
        String::from_utf8(
            self.last_key
                .as_bytes()
                .iter()
                .flat_map(|b| std::ascii::escape_default(*b))
                .collect::<Vec<u8>>(),
        )
        .unwrap()
    }
}

impl Default for SstMetadata {
    fn default() -> Self {
        let mut last_key = Buffer::new(MAX_KEY_LEN);
        for i in 0..MAX_KEY_LEN {
            last_key.as_mut()[i] = 0xffu8;
        }
        Self {
            setsum: [0u8; 32],
            first_key: Buffer::new(0),
            last_key,
            smallest_timestamp: 0,
            biggest_timestamp: u64::max_value(),
            file_size: 0,
        }
    }
}

impl Debug for SstMetadata {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "SstMetadata {{ setsum: {}, first_key: \"{}\", last_key: \"{}\", smallest_timestamp: {} biggest_timestamp: {}, file_size: {} }}",
            self.setsum(), self.first_key_escaped(), self.last_key_escaped(), self.smallest_timestamp, self.biggest_timestamp, self.file_size)
    }
}

//////////////////////////////////////////////// Sst ///////////////////////////////////////////////

#[derive(Clone)]
pub struct Sst {
    // The file backing the table.
    handle: FileHandle,
    // The final block of the table.
    final_block: FinalBlock,
    // Sst metadata.
    index_block: Block,
    // Cache for metadata call.
    file_size: u64,
}

impl Sst {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let handle = open_without_manager(path.as_ref().to_path_buf())?;
        Sst::from_file_handle(handle)
    }

    pub fn from_file_handle(handle: FileHandle) -> Result<Self, Error> {
        // Read and parse the final block's offset
        let file_size = handle.size()?;
        if file_size < 8 {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "file has fewer than eight bytes".to_string(),
            };
            return Err(err);
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
            .with_variable("final_block_offset", final_block_offset)
            .with_variable("file_size", file_size);
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
        // Check that the final block's metadata is sane.
        if final_block.index_block.limit > final_block_offset {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "index_block runs past final_block_offset".to_string(),
            }
            .with_variable("final_block_offset", final_block_offset)
            .with_variable("index_block_limit", final_block.index_block.limit);
            return Err(err);
        }
        let index_block = Sst::load_block(&handle, &final_block.index_block)?;
        Ok(Self {
            handle,
            final_block,
            index_block,
            file_size,
        })
    }

    pub fn cursor(&self) -> SstCursor {
        SstCursor::new(self.clone())
    }

    pub fn setsum(&self) -> Setsum {
        Setsum::from_digest(self.final_block.setsum)
    }

    pub fn metadata(&self) -> Result<SstMetadata, Error> {
        let mut cursor = self.cursor();
        // First key.
        cursor.seek_to_first()?;
        cursor.next()?;
        let kvr = cursor.value();
        let first_key = match kvr {
            Some(kvr) => Buffer::from(kvr.key),
            None => Buffer::new(0),
        };
        // Last key.
        cursor.seek_to_last()?;
        cursor.prev()?;
        let kvr = cursor.value();
        let last_key = match kvr {
            Some(kvr) => Buffer::from(kvr.key),
            None => {
                let mut buf = Buffer::new(MAX_KEY_LEN);
                for i in 0..MAX_KEY_LEN {
                    buf.as_mut()[i] = 0xffu8;
                }
                buf
            }
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

    fn load_block(
        file: &FileHandle,
        block_metadata: &BlockMetadata,
    ) -> Result<Block, Error> {
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
            SstEntry::PlainBlock(bytes) => Ok(Block::new(bytes.into())?),
            SstEntry::FinalBlock(_) => {
                CORRUPTION.click();
                Err(Error::Corruption {
                    core: ErrorCore::default(),
                    context: "tried loading final block".to_string(),
                })
            }
        }
    }
}

///////////////////////////////////////// BlockCompression /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockCompression {
    NoCompression,
}

impl BlockCompression {
    fn compress<'a>(&self, bytes: &'a [u8], scratch: &'a mut Vec<u8>) -> SstEntry<'a> {
        match self {
            BlockCompression::NoCompression => {
                scratch.clear();
                SstEntry::PlainBlock(bytes)
            },
        }
    }
}

//////////////////////////////////////////// SstOptions ////////////////////////////////////////////

pub const CLAMP_MIN_TARGET_BLOCK_SIZE: u32 = 1u32 << 12;
pub const CLAMP_MAX_TARGET_BLOCK_SIZE: u32 = 1u32 << 24;

pub const CLAMP_MIN_TARGET_FILE_SIZE: u32 = 1u32 << 12;
pub const CLAMP_MAX_TARGET_FILE_SIZE: u32 = TABLE_FULL_SIZE as u32;

#[derive(Clone, CommandLine, Debug, Eq, PartialEq)]
pub struct SstOptions {
    #[arrrg(nested)]
    block: BlockBuilderOptions,
    // TODO(rescrv): arrrg needs an enum helper.
    block_compression: BlockCompression,
    #[arrrg(optional, "Target block size.", "BYTES")]
    target_block_size: usize,
    #[arrrg(optional, "Target file size.", "BYTES")]
    target_file_size: usize,
}

impl SstOptions {
    pub fn block(mut self, block: BlockBuilderOptions) -> Self {
        self.block = block;
        self
    }

    pub fn block_compression(mut self, block_compression: BlockCompression) -> Self {
        self.block_compression = block_compression;
        self
    }

    pub fn target_block_size(mut self, mut target_block_size: u32) -> Self {
        if target_block_size < CLAMP_MIN_TARGET_BLOCK_SIZE {
            target_block_size = CLAMP_MIN_TARGET_BLOCK_SIZE;
        }
        if target_block_size > CLAMP_MAX_TARGET_BLOCK_SIZE {
            target_block_size = CLAMP_MAX_TARGET_BLOCK_SIZE;
        }
        self.target_block_size = target_block_size as usize;
        self
    }

    pub fn target_file_size(mut self, mut target_file_size: u32) -> Self {
        if target_file_size < CLAMP_MIN_TARGET_FILE_SIZE {
            target_file_size = CLAMP_MIN_TARGET_FILE_SIZE;
        }
        if target_file_size > CLAMP_MAX_TARGET_FILE_SIZE {
            target_file_size = CLAMP_MAX_TARGET_FILE_SIZE;
        }
        self.target_file_size = target_file_size as usize;
        self
    }
}

impl Default for SstOptions {
    fn default() -> SstOptions {
        SstOptions {
            block: BlockBuilderOptions::default(),
            block_compression: BlockCompression::NoCompression,
            target_block_size: 4096,
            target_file_size: 1<<22,
        }
    }
}

//////////////////////////////////////////// SstBuilder ////////////////////////////////////////////

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
    // The checksum of the file.
    setsum: Setsum,
    // Timestamps seen.
    smallest_timestamp: u64,
    biggest_timestamp: u64,
    // Output information.
    output: File,
    path: PathBuf,
}

impl SstBuilder {
    pub fn new<P: AsRef<Path>>(path: P, options: SstOptions) -> Result<Self, Error> {
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.as_ref())
            .as_z()
            .with_variable("open", path.as_ref().to_string_lossy())?;
        Ok(SstBuilder {
            options,
            last_key: Vec::new(),
            last_timestamp: u64::max_value(),
            block_builder: None,
            block_start_offset: 0,
            bytes_written: 0,
            index_block: BlockBuilder::new(BlockBuilderOptions::default()),
            setsum: Setsum::default(),
            smallest_timestamp: u64::max_value(),
            biggest_timestamp: 0,
            output,
            path: path.as_ref().to_path_buf(),
        })
    }

    fn enforce_sort_order(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        if compare_key(&self.last_key, self.last_timestamp, key, timestamp) != Ordering::Less {
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

    fn get_block(
        &mut self,
        key: &[u8],
        timestamp: u64,
    ) -> Result<&mut BlockBuilder, Error> {
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
        check_key_len(key)?;
        check_value_len(value)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.put(key, timestamp, value)?;
        self.setsum.put(key, timestamp, value);
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.del(key, timestamp)?;
        self.setsum.del(key, timestamp);
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    fn seal(self) -> Result<Sst, Error> {
        let mut builder = self;
        // Flush the block we have.
        if builder.block_builder.is_some() {
            let (key, timestamp) = minimal_successor_key(&builder.last_key, builder.last_timestamp);
            builder.flush_block(&key, timestamp)?;
        }
        // Flush the index block at the end.
        let index_block = builder.index_block.seal()?;
        let index_block_start = builder.bytes_written;
        let bytes = index_block.as_bytes();
        let entry = SstEntry::PlainBlock(bytes);
        let crc32c = entry.crc32c();
        let pa = stack_pack(entry);
        builder.bytes_written += pa.stream(&mut builder.output).as_z()?;
        let index_block_limit = builder.bytes_written;
        // Update timestamps if nothing written
        if builder.smallest_timestamp > builder.biggest_timestamp {
            builder.smallest_timestamp = 0;
            builder.biggest_timestamp = 0;
        }
        // Our final_block
        let final_block = FinalBlock {
            index_block: BlockMetadata {
                start: index_block_start as u64,
                limit: index_block_limit as u64,
                crc32c,
            },
            final_block_offset: builder.bytes_written as u64,
            setsum: builder.setsum.digest(),
            smallest_timestamp: builder.smallest_timestamp,
            biggest_timestamp: builder.biggest_timestamp,
        };
        let pa = stack_pack(final_block);
        builder.bytes_written += pa.stream(&mut builder.output).as_z()?;
        // fsync
        builder.output.sync_all().as_z()?;
        Sst::new(builder.path)
    }
}

////////////////////////////////////////// SstMultiBuilder /////////////////////////////////////////

pub struct SstMultiBuilder {
    prefix: PathBuf,
    suffix: String,
    counter: u64,
    options: SstOptions,
    builder: Option<SstBuilder>,
    paths: Vec<PathBuf>,
}

impl SstMultiBuilder {
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
        let path = self.prefix.join(PathBuf::from(format!("{}{}", self.counter, self.suffix)));
        self.paths.push(path.clone());
        self.counter += 1;
        self.builder = Some(SstBuilder::new(path, self.options.clone())?);
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

///////////////////////////////////////////// SstCursor ////////////////////////////////////////////

pub struct SstCursor {
    table: Sst,
    // The position in the table.  When meta_cursor is at its extremes, block_cursor is None.
    // Otherwise, block_cursor is positioned at the block referred to by the most recent
    // KVP-returning call to meta_cursor.
    meta_cursor: BlockCursor,
    block_cursor: Option<BlockCursor>,
}

impl SstCursor {
    fn new(table: Sst) -> Self {
        let meta_cursor = table.index_block.cursor();
        Self {
            table,
            meta_cursor,
            block_cursor: None,
        }
    }

    fn meta_prev(&mut self) -> Result<Option<BlockMetadata>, Error> {
        self.meta_cursor.prev()?;
        let kvp = match self.meta_cursor.value() {
            Some(kvp) => kvp,
            None => {
                self.seek_to_first()?;
                return Ok(None);
            }
        };
        SstCursor::metadata_from_kvp(kvp)
    }

    fn meta_next(&mut self) -> Result<Option<BlockMetadata>, Error> {
        self.meta_cursor.next()?;
        let kvp = match self.meta_cursor.value() {
            Some(kvp) => kvp,
            None => {
                self.seek_to_last()?;
                return Ok(None);
            }
        };
        SstCursor::metadata_from_kvp(kvp)
    }

    fn metadata_from_kvp(kvr: KeyValueRef) -> Result<Option<BlockMetadata>, Error> {
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

impl Cursor for SstCursor {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.meta_cursor.seek_to_first()?;
        self.block_cursor = None;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.meta_cursor.seek_to_last()?;
        self.block_cursor = None;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        self.meta_cursor.seek(key)?;
        let metadata = match self.meta_next()? {
            Some(m) => m,
            None => {
                return self.seek_to_last();
            }
        };
        let block = Sst::load_block(&self.table.handle, &metadata)?;
        let mut block_cursor = block.cursor();
        block_cursor.seek(key)?;
        self.block_cursor = Some(block_cursor);
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        if self.block_cursor.is_none() {
            let metadata = match self.meta_prev()? {
                Some(m) => m,
                None => {
                    return self.seek_to_first();
                }
            };
            let block = Sst::load_block(&self.table.handle, &metadata)?;
            let mut block_cursor = block.cursor();
            block_cursor.seek_to_last()?;
            self.block_cursor = Some(block_cursor);
        }
        assert!(self.block_cursor.is_some());
        let block_cursor: &mut BlockCursor = self.block_cursor.as_mut().unwrap();
        block_cursor.prev()?;
        match block_cursor.value() {
            Some(_) => Ok(()),
            None => {
                self.block_cursor = None;
                self.prev()
            }
        }
    }

    fn next(&mut self) -> Result<(), Error> {
        if self.block_cursor.is_none() {
            let metadata = match self.meta_next()? {
                Some(m) => m,
                None => {
                    return self.seek_to_last();
                }
            };
            let block = Sst::load_block(&self.table.handle, &metadata)?;
            let mut block_cursor = block.cursor();
            block_cursor.seek_to_first()?;
            self.block_cursor = Some(block_cursor);
        }
        assert!(self.block_cursor.is_some());
        let block_cursor: &mut BlockCursor = self.block_cursor.as_mut().unwrap();
        block_cursor.next()?;
        match block_cursor.value() {
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

    fn value(&self) -> Option<KeyValueRef> {
        match &self.block_cursor {
            Some(cursor) => cursor.value(),
            None => None,
        }
    }
}

impl From<Sst> for SstCursor {
    fn from(table: Sst) -> Self {
        Self::new(table)
    }
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

// Content under CC By-Sa.  I just use as is, as can you.
// https://codereview.stackexchange.com/questions/233872/writing-slice-compare-in-a-more-compact-way
pub fn compare_bytes(a: &[u8], b: &[u8]) -> cmp::Ordering {
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
        d_key.extend_from_slice(&key_lhs[0..shared + 1]);
        d_key[shared] = key_lhs[shared] + 1;
        d_timestamp = 0;
    } else {
        d_key.extend_from_slice(key_lhs);
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
