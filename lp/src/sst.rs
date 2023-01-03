use std::cmp::Ordering;
use std::fmt::Write;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use crc32c;

use buffertk::{stack_pack, Buffer, Packable, Unpacker};

use setsum::Setsum;

use super::block::{Block, BlockBuilder, BlockBuilderOptions, BlockCursor};
use super::file_manager::{open_without_manager, FileHandle};
use super::{
    check_key_len, check_table_size, check_value_len, compare_key, divide_keys,
    minimal_successor_key, Builder, Cursor, Error, KeyValueRef, MAX_KEY_LEN,
};

///////////////////////////////////////////// SSTEntry /////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum SSTEntry<'a> {
    #[prototk(10, bytes)]
    PlainBlock(&'a [u8]),
    // #[prototk(11, bytes)]
    // ZstdBlock(&'a [u8]),
    #[prototk(12, bytes)]
    FinalBlock(&'a [u8]),
}

impl<'a> SSTEntry<'a> {
    fn bytes(&self) -> &[u8] {
        match self {
            SSTEntry::PlainBlock(x) => x,
            SSTEntry::FinalBlock(x) => x,
        }
    }

    fn crc32c(&self) -> u32 {
        crc32c::crc32c(self.bytes())
    }
}

impl<'a> Default for SSTEntry<'a> {
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
            return Err(Error::Corruption {
                context: format!(
                    "block_metadata.start={} >= block_metadata.limit={}",
                    self.start, self.limit
                ),
            });
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

//////////////////////////////////////////// SSTMetadata ///////////////////////////////////////////

#[derive(Clone, Debug, Message)]
pub struct SSTMetadata {
    #[prototk(1, bytes32)]
    pub setsum: [u8; 32],
    #[prototk(2, buffer)]
    pub first_key: Buffer,
    #[prototk(3, buffer)]
    pub last_key: Buffer,
    #[prototk(4, uint64)]
    pub smallest_timestamp: u64,
    #[prototk(5, uint64)]
    pub biggest_timestamp: u64,
}

impl SSTMetadata {
    // TODO(rescrv): dedupe with the other implementations.
    pub fn setsum(&self) -> String {
        let mut setsum = String::with_capacity(68);
        for i in 0..self.setsum.len() {
            write!(&mut setsum, "{:02x}", self.setsum[i])
                .expect("unable to write to string");
        }
        setsum
    }

    pub fn first_key_escaped(&self) -> String {
        String::from_utf8(self.first_key.as_bytes().iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap()
    }

    pub fn last_key_escaped(&self) -> String {
        String::from_utf8(self.last_key.as_bytes().iter().flat_map(|b| std::ascii::escape_default(*b)).collect::<Vec<u8>>()).unwrap()
    }
}

impl Default for SSTMetadata {
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
        }
    }
}

//////////////////////////////////////////////// SST ///////////////////////////////////////////////

#[derive(Clone)]
pub struct SST {
    // The file backing the table.
    handle: FileHandle,
    // The final block of the table.
    final_block: FinalBlock,
    // SST metadata.
    index_block: Block,
}

impl SST {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let handle = open_without_manager(path.as_ref().to_path_buf())?;
        SST::from_file_handle(handle)
    }

    pub fn from_file_handle(handle: FileHandle) -> Result<Self, Error> {
        // Read and parse the final block's offset
        let file_sz = handle.size()?;
        if file_sz < 8 {
            return Err(Error::Corruption {
                context: "file has fewer than eight bytes".to_string(),
            });
        }
        let position = file_sz - 8;
        let mut buf: Vec<u8> = vec![0, 0, 0, 0, 0, 0, 0, 0];
        handle.read_exact_at(&mut buf, position)?;
        let mut up = Unpacker::new(&buf);
        let final_block_offset: u64 =
            up.unpack()
                .map_err(|e: buffertk::Error| Error::UnpackError {
                    error: e.into(),
                    context: "parsing final block offset".to_string(),
                })?;
        // Read and parse the final block
        if file_sz < final_block_offset {
            return Err(Error::Corruption {
                context: format!(
                    "final block offset reported at {}, but file is {} bytes",
                    final_block_offset, file_sz
                ),
            });
        }
        let size_of_final_block = position + 8 - (final_block_offset);
        buf.resize(size_of_final_block as usize, 0);
        handle.read_exact_at(&mut buf, final_block_offset)?;
        let mut up = Unpacker::new(&buf);
        let final_block: FinalBlock = up.unpack().map_err(|e| Error::UnpackError {
            error: e,
            context: "parsing final block".to_string(),
        })?;
        final_block.index_block.sanity_check()?;
        // Check that the final block's metadata is sane.
        if final_block.index_block.limit > final_block_offset {
            return Err(Error::Corruption {
                context: format!(
                    "index_block runs past final_block_offset={} limit={}",
                    final_block_offset, final_block.index_block.limit
                ),
            });
        }
        let index_block = SST::load_block(&handle, &final_block.index_block)?;
        Ok(Self {
            handle,
            final_block,
            index_block,
        })
    }

    pub fn iterate(&self) -> SSTCursor {
        SSTCursor::new(self.clone())
    }

    pub fn setsum(&self) -> String {
        let mut setsum = String::with_capacity(68);
        for i in 0..self.final_block.setsum.len() {
            write!(&mut setsum, "{:02x}", self.final_block.setsum[i])
                .expect("unable to write to string");
        }
        setsum
    }

    pub fn metadata(&self) -> Result<SSTMetadata, Error> {
        let mut cursor = self.iterate();
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
        Ok(SSTMetadata {
            setsum: self.final_block.setsum,
            first_key,
            last_key,
            smallest_timestamp: self.final_block.smallest_timestamp,
            biggest_timestamp: self.final_block.biggest_timestamp,
        })
    }

    fn load_block(file: &FileHandle, block_metadata: &BlockMetadata) -> Result<Block, Error> {
        block_metadata.sanity_check()?;
        let amt = (block_metadata.limit - block_metadata.start) as usize;
        let mut buf: Vec<u8> = Vec::with_capacity(amt);
        buf.resize(amt, 0);
        file.read_exact_at(&mut buf, block_metadata.start)?;
        let mut up = Unpacker::new(&buf);
        let table_entry: SSTEntry = up.unpack().map_err(|e| Error::UnpackError {
            error: e,
            context: "parsing table entry".to_string(),
        })?;
        if table_entry.crc32c() != block_metadata.crc32c {
            return Err(Error::CRC32CFailure {
                start: block_metadata.start,
                limit: block_metadata.limit,
                crc32c: block_metadata.crc32c,
            });
        }
        match table_entry {
            SSTEntry::PlainBlock(bytes) => Ok(Block::new(bytes.into())?),
            SSTEntry::FinalBlock(_) => Err(Error::Corruption {
                context: "tried loading final block".to_string(),
            }),
        }
    }
}

///////////////////////////////////////// BlockCompression /////////////////////////////////////////

#[derive(Clone)]
pub enum BlockCompression {
    NoCompression,
}

impl BlockCompression {
    fn compress<'a>(&self, bytes: &'a [u8], _scratch: &'a mut Vec<u8>) -> SSTEntry<'a> {
        match self {
            BlockCompression::NoCompression => SSTEntry::PlainBlock(bytes),
        }
    }
}

///////////////////////////////////////// SSTBuilderOptions ////////////////////////////////////////

pub const CLAMP_MIN_TARGET_BLOCK_SIZE: u32 = 1u32 << 12;
pub const CLAMP_MAX_TARGET_BLOCK_SIZE: u32 = 1u32 << 24;

#[derive(Clone)]
pub struct SSTBuilderOptions {
    block_options: BlockBuilderOptions,
    block_compression: BlockCompression,
    target_block_size: usize,
}

impl SSTBuilderOptions {
    pub fn block_options(mut self, block_options: BlockBuilderOptions) -> Self {
        self.block_options = block_options;
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
}

impl Default for SSTBuilderOptions {
    fn default() -> SSTBuilderOptions {
        SSTBuilderOptions {
            block_options: BlockBuilderOptions::default(),
            block_compression: BlockCompression::NoCompression,
            target_block_size: 4096,
        }
    }
}

//////////////////////////////////////////// SSTBuilder ////////////////////////////////////////////

pub struct SSTBuilder {
    // Options for every "normal" table entry.
    options: SSTBuilderOptions,
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

impl SSTBuilder {
    pub fn new<P: AsRef<Path>>(path: P, options: SSTBuilderOptions) -> Result<Self, Error> {
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.as_ref().to_path_buf())?;
        Ok(SSTBuilder {
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
        if !self.block_builder.is_none() {
            return Err(Error::LogicError {
                context: "called start_new_block() when block_builder is not None".to_string(),
            });
        }
        self.block_builder = Some(BlockBuilder::new(self.options.block_options.clone()));
        self.block_start_offset = self.bytes_written;
        Ok(())
    }

    fn flush_block(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        if !self.block_builder.is_some() {
            return Err(Error::LogicError {
                context: "self.block_builder.is_none()".to_string(),
            });
        }
        // Metadata for the block.
        let mut block_metadata = BlockMetadata::default();
        block_metadata.start = self.bytes_written as u64;
        // Write out the block.
        let block = self.block_builder.take().unwrap().seal()?;
        let bytes = block.as_bytes();
        let mut scratch = Vec::new();
        let entry = self.options.block_compression.compress(bytes, &mut scratch);
        block_metadata.crc32c = entry.crc32c();
        let pa = stack_pack(entry);
        self.bytes_written += pa.stream(&mut self.output)?;
        // Prepare the block metadata.
        block_metadata.limit = self.bytes_written as u64;
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

impl Builder for SSTBuilder {
    type Sealed = SST;

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
        self.setsum
            .insert_vectored(&[&[8], key, &timestamp.to_le_bytes(), value]);
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.del(key, timestamp)?;
        self.setsum
            .insert_vectored(&[&[9], key, &timestamp.to_le_bytes(), &[]]);
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    fn seal(self) -> Result<SST, Error> {
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
        let entry = SSTEntry::PlainBlock(bytes);
        let crc32c = entry.crc32c();
        let pa = stack_pack(entry);
        builder.bytes_written += pa.stream(&mut builder.output)?;
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
        builder.bytes_written += pa.stream(&mut builder.output)?;
        // fsync
        builder.output.sync_all()?;
        Ok(SST::new(builder.path)?)
    }
}

///////////////////////////////////////////// SSTCursor ////////////////////////////////////////////

pub struct SSTCursor {
    table: SST,
    // The position in the table.  When meta_iter is at its extremes, block_iter is None.
    // Otherwise, block_iter is positioned at the block referred to by the most recent
    // KVP-returning call to meta_iter.
    meta_iter: BlockCursor,
    block_iter: Option<BlockCursor>,
}

impl SSTCursor {
    fn new(table: SST) -> Self {
        let meta_iter = table.index_block.iterate();
        Self {
            table,
            meta_iter,
            block_iter: None,
        }
    }

    fn meta_prev(&mut self) -> Result<Option<BlockMetadata>, Error> {
        self.meta_iter.prev()?;
        let kvp = match self.meta_iter.value() {
            Some(kvp) => kvp,
            None => {
                self.seek_to_first()?;
                return Ok(None);
            }
        };
        SSTCursor::metadata_from_kvp(kvp)
    }

    fn meta_next(&mut self) -> Result<Option<BlockMetadata>, Error> {
        self.meta_iter.next()?;
        let kvp = match self.meta_iter.value() {
            Some(kvp) => kvp,
            None => {
                self.seek_to_last()?;
                return Ok(None);
            }
        };
        SSTCursor::metadata_from_kvp(kvp)
    }

    fn metadata_from_kvp(kvr: KeyValueRef) -> Result<Option<BlockMetadata>, Error> {
        let value = match kvr.value {
            Some(v) => v,
            None => {
                return Err(Error::Corruption {
                    context: "meta block has null value".to_string(),
                });
            }
        };
        let mut up = Unpacker::new(value);
        let metadata: BlockMetadata = up.unpack().map_err(|e| Error::UnpackError {
            error: e,
            context: "parsing block metadata".to_string(),
        })?;
        Ok(Some(metadata))
    }
}

impl Cursor for SSTCursor {
    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.meta_iter.seek_to_first()?;
        self.block_iter = None;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.meta_iter.seek_to_last()?;
        self.block_iter = None;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        self.meta_iter.seek(key)?;
        let metadata = match self.meta_next()? {
            Some(m) => m,
            None => {
                return self.seek_to_last();
            }
        };
        let block = SST::load_block(&self.table.handle, &metadata)?;
        let mut block_iter = block.iterate();
        block_iter.seek(key)?;
        self.block_iter = Some(block_iter);
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        if self.block_iter.is_none() {
            let metadata = match self.meta_prev()? {
                Some(m) => m,
                None => {
                    return self.seek_to_first();
                }
            };
            let block = SST::load_block(&self.table.handle, &metadata)?;
            let mut block_iter = block.iterate();
            block_iter.seek_to_last()?;
            self.block_iter = Some(block_iter);
        }
        assert!(self.block_iter.is_some());
        let block_iter: &mut BlockCursor = self.block_iter.as_mut().unwrap();
        block_iter.prev()?;
        match block_iter.value() {
            Some(_) => Ok(()),
            None => {
                self.block_iter = None;
                self.prev()
            }
        }
    }

    fn next(&mut self) -> Result<(), Error> {
        if self.block_iter.is_none() {
            let metadata = match self.meta_next()? {
                Some(m) => m,
                None => {
                    return self.seek_to_last();
                }
            };
            let block = SST::load_block(&self.table.handle, &metadata)?;
            let mut block_iter = block.iterate();
            block_iter.seek_to_first()?;
            self.block_iter = Some(block_iter);
        }
        assert!(self.block_iter.is_some());
        let block_iter: &mut BlockCursor = self.block_iter.as_mut().unwrap();
        block_iter.next()?;
        match block_iter.value() {
            Some(_) => Ok(()),
            None => {
                self.block_iter = None;
                self.next()
            }
        }
    }

    fn value(&self) -> Option<KeyValueRef> {
        match &self.block_iter {
            Some(iter) => iter.value(),
            None => None,
        }
    }
}

impl From<SST> for SSTCursor {
    fn from(table: SST) -> Self {
        Self::new(table)
    }
}
