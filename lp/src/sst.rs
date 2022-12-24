use std::cmp::Ordering;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use prototk::{stack_pack, Packable, Unpacker};

use super::block::{Block, BlockBuilder, BlockBuilderOptions, BlockCursor};
use super::file_manager::{open_without_manager, FileHandle};
use super::{
    check_key_len, check_table_size, check_value_len, compare_key, divide_keys,
    minimal_successor_key, Builder, Cursor, Error, KeyValuePair,
};

///////////////////////////////////////////// SSTEntry /////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum SSTEntry<'a> {
    #[prototk(10, bytes)]
    NOP(&'a [u8]),
    #[prototk(11, bytes)]
    PlainBlock(&'a [u8]),
    // #[prototk(12, bytes)]
    // ZstdBlock(&'a [u8]),
    #[prototk(13, bytes)]
    FinalBlock(&'a [u8]),
}

impl<'a> Default for SSTEntry<'a> {
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

//////////////////////////////////////////////// SST ///////////////////////////////////////////////

#[derive(Clone)]
pub struct SST {
    // The file backing the table.
    handle: FileHandle,
    // SST metadata.
    index_block: Block,
}

impl SST {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let handle = open_without_manager(path)?;
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
        let index_block = SST::load_block(&handle, &final_block.index_block)?;
        Ok(Self {
            handle,
            index_block,
        })
    }

    pub fn iterate(&self) -> SSTCursor {
        SSTCursor::new(self.clone())
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
        match table_entry {
            SSTEntry::NOP(_) => {
                Err(Error::Corruption {
                    context: "file has a NOP block".to_string(),
                })
            },
            SSTEntry::PlainBlock(bytes) => {
                Ok(Block::new(bytes.into())?)
            },
            SSTEntry::FinalBlock(_) => {
                Err(Error::Corruption {
                    context: "tried loading final block".to_string(),
                })
            }
        }
    }
}

///////////////////////////////////////// BlockCompression /////////////////////////////////////////

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
    // Output information.
    output: File,
    path: PathBuf,
}

impl SSTBuilder {
    pub fn new(path: PathBuf, options: SSTBuilderOptions) -> Result<Self, Error> {
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path.clone())?;
        Ok(SSTBuilder {
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
        self.assign_last_key(key, timestamp);
        Ok(())
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let block = self.get_block(key, timestamp)?;
        block.del(key, timestamp)?;
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
        let kvp = match self.meta_iter.prev()? {
            Some(kvp) => { kvp },
            None => {
                self.seek_to_first()?;
                return Ok(None);
            },
        };
        SSTCursor::metadata_from_kvp(kvp)
    }

    fn meta_next(&mut self) -> Result<Option<BlockMetadata>, Error> {
        let kvp = match self.meta_iter.next()? {
            Some(kvp) => { kvp },
            None => {
                self.seek_to_last()?;
                return Ok(None);
            },
        };
        SSTCursor::metadata_from_kvp(kvp)
    }

    fn metadata_from_kvp(kvp: KeyValuePair) -> Result<Option<BlockMetadata>, Error> {
        let value = match kvp.value {
            Some(v) => { v },
            None => {
                return Err(Error::Corruption {
                    context: "meta block has null value".to_string(),
                });
            },
        };
        let mut up = Unpacker::new(value.as_bytes());
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

    fn seek(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        self.meta_iter.seek(key, timestamp)?;
        let metadata = match self.meta_next()? {
            Some(m) => { m },
            None => {
                return self.seek_to_last();
            },
        };
        let block = SST::load_block(&self.table.handle, &metadata)?;
        let mut block_iter = block.iterate();
        block_iter.seek(key, timestamp)?;
        self.block_iter = Some(block_iter);
        Ok(())
    }

    fn prev(&mut self) -> Result<Option<KeyValuePair>, Error> {
        if self.block_iter.is_none() {
            let metadata = match self.meta_prev()? {
                Some(m) => { m },
                None => {
                    self.seek_to_first()?;
                    return Ok(None);
                },
            };
            let block = SST::load_block(&self.table.handle, &metadata)?;
            let mut block_iter = block.iterate();
            block_iter.seek_to_last()?;
            self.block_iter = Some(block_iter);
        }
        assert!(self.block_iter.is_some());
        let block_iter: &mut BlockCursor = self.block_iter.as_mut().unwrap();
        match block_iter.prev()? {
            Some(kvp) => { Ok(Some(kvp)) },
            None => {
                self.block_iter = None;
                self.prev()
            }
        }
    }

    fn next(&mut self) -> Result<Option<KeyValuePair>, Error> {
        if self.block_iter.is_none() {
            let metadata = match self.meta_next()? {
                Some(m) => { m },
                None => {
                    self.seek_to_last()?;
                    return Ok(None);
                },
            };
            let block = SST::load_block(&self.table.handle, &metadata)?;
            let mut block_iter = block.iterate();
            block_iter.seek_to_first()?;
            self.block_iter = Some(block_iter);
        }
        assert!(self.block_iter.is_some());
        let block_iter: &mut BlockCursor = self.block_iter.as_mut().unwrap();
        match block_iter.next()? {
            Some(kvp) => { Ok(Some(kvp)) },
            None => {
                self.block_iter = None;
                self.next()
            }
        }
    }
}

#[cfg(test)]
mod alphabet {
    use std::fs::remove_file;

    use super::*;

    fn alphabet(path: PathBuf) -> SST {
        let builder_opts = SSTBuilderOptions::default()
            .block_options(BlockBuilderOptions::default())
            .block_compression(BlockCompression::NoCompression)
            .target_block_size(4096);
        let mut builder = SSTBuilder::new(path, builder_opts).unwrap();
        builder.put("A".as_bytes(), 0, "a".as_bytes()).unwrap();
        builder.put("B".as_bytes(), 0, "b".as_bytes()).unwrap();
        builder.put("C".as_bytes(), 0, "c".as_bytes()).unwrap();
        builder.put("D".as_bytes(), 0, "d".as_bytes()).unwrap();
        builder.put("E".as_bytes(), 0, "e".as_bytes()).unwrap();
        builder.put("F".as_bytes(), 0, "f".as_bytes()).unwrap();
        builder.put("G".as_bytes(), 0, "g".as_bytes()).unwrap();
        builder.put("H".as_bytes(), 0, "h".as_bytes()).unwrap();
        builder.put("I".as_bytes(), 0, "i".as_bytes()).unwrap();
        builder.put("J".as_bytes(), 0, "j".as_bytes()).unwrap();
        builder.put("K".as_bytes(), 0, "k".as_bytes()).unwrap();
        builder.put("L".as_bytes(), 0, "l".as_bytes()).unwrap();
        builder.put("M".as_bytes(), 0, "m".as_bytes()).unwrap();
        builder.put("N".as_bytes(), 0, "n".as_bytes()).unwrap();
        builder.put("O".as_bytes(), 0, "o".as_bytes()).unwrap();
        builder.put("P".as_bytes(), 0, "p".as_bytes()).unwrap();
        builder.put("Q".as_bytes(), 0, "q".as_bytes()).unwrap();
        builder.put("R".as_bytes(), 0, "r".as_bytes()).unwrap();
        builder.put("S".as_bytes(), 0, "s".as_bytes()).unwrap();
        builder.put("T".as_bytes(), 0, "t".as_bytes()).unwrap();
        builder.put("U".as_bytes(), 0, "u".as_bytes()).unwrap();
        builder.put("V".as_bytes(), 0, "v".as_bytes()).unwrap();
        builder.put("W".as_bytes(), 0, "w".as_bytes()).unwrap();
        builder.put("X".as_bytes(), 0, "x".as_bytes()).unwrap();
        builder.put("Y".as_bytes(), 0, "y".as_bytes()).unwrap();
        builder.put("Z".as_bytes(), 0, "z".as_bytes()).unwrap();
        builder.seal().unwrap()
    }

    #[test]
    fn step_the_alphabet_forward() {
        const FILENAME: &str = "step_the_alphabet_forward.sst";
        let table = alphabet(FILENAME.into());
        let mut iter = table.iterate();
        remove_file::<PathBuf>(FILENAME.into()).unwrap();
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // C
        let exp = KeyValuePair {
            key: "C".into(),
            timestamp: 0,
            value: Some("c".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // D-W
        for _ in 0..20 {
            let _got = iter.next().unwrap().unwrap();
        }
        // X
        let exp = KeyValuePair {
            key: "X".into(),
            timestamp: 0,
            value: Some("x".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
    }

    #[test]
    fn step_the_alphabet_reverse() {
        const FILENAME: &str = "step_the_alphabet_reverse.sst";
        let table = alphabet(FILENAME.into());
        let mut iter = table.iterate();
        remove_file::<PathBuf>(FILENAME.into()).unwrap();
        iter.seek_to_last().unwrap();
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // X
        let exp = KeyValuePair {
            key: "X".into(),
            timestamp: 0,
            value: Some("x".into()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // W-D
        for _ in 0..20 {
            let _got = iter.prev().unwrap().unwrap();
        }
        // C
        let exp = KeyValuePair {
            key: "C".into(),
            timestamp: 0,
            value: Some("c".into()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.prev().unwrap().unwrap();
        assert_eq!(exp, got);
        // Last
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
    }

    #[test]
    fn seek_to_at() {
        const FILENAME: &str = "seek_to_at.sst";
        let table = alphabet(FILENAME.into());
        let mut iter = table.iterate();
        remove_file::<PathBuf>(FILENAME.into()).unwrap();
        iter.seek("@".as_bytes(), 0).unwrap();
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
    }

    #[test]
    fn seek_to_z() {
        const FILENAME: &str = "seek_to_z.sst";
        let table = alphabet(FILENAME.into());
        let mut iter = table.iterate();
        remove_file::<PathBuf>(FILENAME.into()).unwrap();
        iter.seek("Z".as_bytes(), 0).unwrap();
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.next().unwrap().unwrap();
        assert_eq!(exp, got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
    }

    #[test]
    fn two_steps_forward_one_step_reverse() {
        const FILENAME: &str = "two_steps_forward_one_step_reverse.sst";
        let table = alphabet(FILENAME.into());
        let mut iter = table.iterate();
        remove_file::<PathBuf>(FILENAME.into()).unwrap();
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // C
        let exp = KeyValuePair {
            key: "C".into(),
            timestamp: 0,
            value: Some("c".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // D-W
        for _ in 0..21 {
            iter.next().unwrap();
            iter.next().unwrap();
            iter.prev().unwrap();
        }
        // X
        let exp = KeyValuePair {
            key: "X".into(),
            timestamp: 0,
            value: Some("x".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // X
        let exp = KeyValuePair {
            key: "X".into(),
            timestamp: 0,
            value: Some("x".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
        // Last
        let got = iter.next().unwrap();
        assert_eq!(None, got);
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
    }

    #[test]
    fn two_steps_reverse_one_step_forward() {
        const FILENAME: &str = "two_steps_reverse_one_step_forward.sst";
        let table = alphabet(FILENAME.into());
        let mut iter = table.iterate();
        remove_file::<PathBuf>(FILENAME.into()).unwrap();
        iter.seek_to_last().unwrap();
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Z
        let exp = KeyValuePair {
            key: "Z".into(),
            timestamp: 0,
            value: Some("z".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // X
        let exp = KeyValuePair {
            key: "X".into(),
            timestamp: 0,
            value: Some("x".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // Y
        let exp = KeyValuePair {
            key: "Y".into(),
            timestamp: 0,
            value: Some("y".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // W-D
        for _ in 0..21 {
            iter.prev().unwrap();
            iter.prev().unwrap();
            iter.next().unwrap();
        }
        // C
        let exp = KeyValuePair {
            key: "C".into(),
            timestamp: 0,
            value: Some("c".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // C
        let exp = KeyValuePair {
            key: "C".into(),
            timestamp: 0,
            value: Some("c".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // B
        let exp = KeyValuePair {
            key: "B".into(),
            timestamp: 0,
            value: Some("b".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.prev().unwrap();
        assert_eq!(Some(exp), got);
        // First
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
        // First
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
        // First
        let got = iter.prev().unwrap();
        assert_eq!(None, got);
        // A
        let exp = KeyValuePair {
            key: "A".into(),
            timestamp: 0,
            value: Some("a".into()),
        };
        let got = iter.next().unwrap();
        assert_eq!(Some(exp), got);
    }
}
