use std::cmp::Ordering;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use prototk::{stack_pack, Packable, Unpacker};

use super::block::{Block, BlockBuilder, BlockBuilderOptions, BlockCursor};
use super::file_manager::{open_without_manager, FileHandle};
use super::{
    check_key_len, check_table_size, check_value_len, compare_key, divide_keys,
    minimal_successor_key, Error, KeyValuePair,
};

//////////////////////////////////////////// TableEntry ////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum TableEntry<'a> {
    #[prototk(10, bytes)]
    NOP(&'a [u8]),
    #[prototk(11, bytes)]
    PlainBlock(&'a [u8]),
    // #[prototk(12, bytes)]
    // ZstdBlock(&'a [u8]),
    #[prototk(13, bytes)]
    FinalBlock(&'a [u8]),
}

impl<'a> Default for TableEntry<'a> {
    fn default() -> Self {
        Self::NOP(&[])
    }
}

/////////////////////////////////////////// BlockMetadata //////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct BlockMetadata {
    #[prototk(14, uint64)]
    start: u64,
    #[prototk(15, uint64)]
    limit: u64,
    // NOTE(rescrv): If adding a field, update the constant for max size.
}

const BLOCK_METADATA_MAX_SZ: usize = 22;

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
    #[prototk(18, fixed64)]
    final_block_offset: u64,
    // NOTE(rescrv): If adding a field, update the constant for max size.
}

const FINAL_BLOCK_MAX_SZ: usize = 2 + BLOCK_METADATA_MAX_SZ + 2 + 8;

/////////////////////////////////////////////// Table //////////////////////////////////////////////

pub struct Table {
    // The file backing the table.
    handle: FileHandle,
    // Table metadata.
    index_block: Block,
}

impl Table {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let handle = open_without_manager(path)?;
        Table::from_file_handle(handle)
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
        let final_block_offset: u64 = up.unpack().map_err(|e| Error::UnpackError {
            error: e,
            context: "parsing final block offset".to_string(),
        })?;
        // Read and parse the final block
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
        let index_block = Table::load_block(&handle, &final_block.index_block)?;
        Ok(Self {
            handle,
            index_block,
        })
    }

    pub fn iterate<'a>(&'a self) -> TableCursor<'a> {
        TableCursor::new(self)
    }

    fn load_block(file: &FileHandle, block_metadata: &BlockMetadata) -> Result<Block, Error> {
        block_metadata.sanity_check()?;
        let amt = (block_metadata.limit - block_metadata.start) as usize;
        let mut buf: Vec<u8> = Vec::with_capacity(amt);
        buf.resize(amt, 0);
        file.read_exact_at(&mut buf, block_metadata.start)?;
        Ok(Block::new(buf.into())?)
    }
}

///////////////////////////////////////// BlockCompression /////////////////////////////////////////

pub enum BlockCompression {
    NoCompression,
}

impl BlockCompression {
    fn compress<'a>(&self, bytes: &'a [u8], _scratch: &'a mut Vec<u8>) -> TableEntry<'a> {
        match self {
            BlockCompression::NoCompression => TableEntry::PlainBlock(bytes),
        }
    }
}

//////////////////////////////////////// TableBuilderOptions ///////////////////////////////////////

pub const CLAMP_MIN_TARGET_BLOCK_SIZE: u32 = 1u32 << 12;
pub const CLAMP_MAX_TARGET_BLOCK_SIZE: u32 = 1u32 << 24;

pub struct TableBuilderOptions {
    block_options: BlockBuilderOptions,
    block_compression: BlockCompression,
    target_block_size: usize,
}

impl TableBuilderOptions {
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

impl Default for TableBuilderOptions {
    fn default() -> TableBuilderOptions {
        TableBuilderOptions {
            block_options: BlockBuilderOptions::default(),
            block_compression: BlockCompression::NoCompression,
            target_block_size: 4096,
        }
    }
}

/////////////////////////////////////////// TableBuilder ///////////////////////////////////////////

pub struct TableBuilder {
    // Options for every "normal" table entry.
    options: TableBuilderOptions,
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
    // Output information.
    output: File,
    path: PathBuf,
}

impl TableBuilder {
    pub fn new(path: PathBuf, options: TableBuilderOptions) -> Result<Self, Error> {
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.clone())?;
        Ok(TableBuilder {
            options,
            last_key: Vec::new(),
            last_timestamp: u64::max_value(),
            block_builder: None,
            block_start_offset: 0,
            bytes_written: 0,
            index_block: BlockBuilder::new(BlockBuilderOptions::default()),
            output,
            path,
        })
    }

    pub fn approximate_size(&self) -> usize {
        let mut sum = self.bytes_written;
        sum += match &self.block_builder {
            Some(block) => block.approximate_size(),
            None => 0,
        };
        sum += 1 + self.index_block.approximate_size();
        sum += FINAL_BLOCK_MAX_SZ;
        sum
    }

    pub fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        check_key_len(key)?;
        check_value_len(value)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.put(key, timestamp, value)?;
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    pub fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.del(key, timestamp)?;
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    pub fn seal(self) -> Result<Table, Error> {
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
        let entry = TableEntry::PlainBlock(bytes);
        let pa = stack_pack(entry);
        builder.bytes_written += pa.stream(&mut builder.output)?;
        let index_block_limit = builder.bytes_written;
        // Our final_block
        let final_block = FinalBlock {
            index_block: BlockMetadata {
                start: index_block_start as u64,
                limit: index_block_limit as u64,
            },
            final_block_offset: builder.bytes_written as u64,
        };
        let pa = stack_pack(final_block);
        builder.bytes_written += pa.stream(&mut builder.output)?;
        // fsync
        builder.output.sync_all()?;
        Ok(Table::new(builder.path)?)
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

//////////////////////////////////////////// TableCursor ///////////////////////////////////////////

pub struct TableCursor<'a> {
    table: &'a Table,
    // The position in the table.  When meta_iter is at its extremes, block_iter is None.
    // Otherwise, block_iter is positioned at the block referred to by the most recent
    // KVP-returning call to meta_iter.
    meta_iter: BlockCursor<'a>,
    block_iter: Option<BlockCursor<'a>>,
}

impl<'a> TableCursor<'a> {
    fn new(table: &'a Table) -> Self {
        let meta_iter = table.index_block.iterate();
        Self {
            table,
            meta_iter,
            block_iter: None,
        }
    }

    pub fn seek_to_first(&mut self) -> Result<(), Error> {
        self.meta_iter.seek_to_first()?;
        self.block_iter = None;
        Ok(())
    }

    pub fn seek_to_last(&mut self) -> Result<(), Error> {
        self.meta_iter.seek_to_last()?;
        self.block_iter = None;
        Ok(())
    }

    pub fn seek(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        todo!();
    }

    pub fn prev(&mut self) -> Result<Option<KeyValuePair>, Error> {
        todo!();
    }

    pub fn next(&mut self) -> Result<Option<KeyValuePair>, Error> {
        todo!();
    }
}
