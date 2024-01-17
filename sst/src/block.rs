//! A block is the base unit of an SST.  This module provides implementations of cursors and
//! builders for blocks.

use std::cmp;
use std::cmp::Ordering;
use std::ops::Bound;
use std::sync::Arc;

use buffertk::{length_free, stack_pack, v64, Packable, Unpacker};
use keyvalint::{compare_bytes, compare_key, Cursor, KeyRef};
use zerror::Z;
use zerror_core::ErrorCore;

use super::{
    check_key_len, check_table_size, check_value_len, Builder, Error, KeyValueDel, KeyValueEntry,
    KeyValuePut, CORRUPTION, LOGIC_ERROR,
};
use crate::bounds_cursor::BoundsCursor;
use crate::pruning_cursor::PruningCursor;

//////////////////////////////////////// BlockBuilderOptions ///////////////////////////////////////

/// Options for building blocks.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct BlockBuilderOptions {
    /// Store a complete key every bytes_restart_interval bytes.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Store a complete key every this many bytes.", "BYTES")
    )]
    bytes_restart_interval: u64,
    /// Store a complete key every key_value_pairs_restart_interval keys.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Store a complete key every this many keys.", "KEYS")
    )]
    key_value_pairs_restart_interval: u64,
}

impl BlockBuilderOptions {
    /// Set the bytes_restart_interval.
    pub fn bytes_restart_interval(mut self, bytes_restart_interval: u32) -> Self {
        self.bytes_restart_interval = bytes_restart_interval as u64;
        self
    }

    /// Set the key_value_pairs_restart_interval.
    pub fn key_value_pairs_restart_interval(
        mut self,
        key_value_pairs_restart_interval: u32,
    ) -> Self {
        self.key_value_pairs_restart_interval = key_value_pairs_restart_interval as u64;
        self
    }
}

impl Default for BlockBuilderOptions {
    fn default() -> Self {
        Self {
            bytes_restart_interval: 1024,
            key_value_pairs_restart_interval: 16,
        }
    }
}

/////////////////////////////////////////////// Block //////////////////////////////////////////////

/// A Block captures an immutable, sorted sequence of key-value pairs.
#[derive(Clone, Debug)]
pub struct Block {
    // The raw bytes built by a builder or loaded off disk.
    bytes: Arc<Vec<u8>>,

    // The restart intervals.  restarts_boundary points to the first restart point.
    restarts_boundary: usize,
    restarts_idx: usize,
    num_restarts: usize,
}

impl Block {
    /// Create a new block from the provided bytes.
    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        // Load num_restarts.
        let bytes = Arc::new(bytes);
        if bytes.len() < 4 {
            // This is impossible.  A block must end in a u32 that indicates how many restarts
            // there are.
            return Err(Error::BlockTooSmall {
                core: ErrorCore::default(),
                length: bytes.len(),
                required: 4,
            });
        }
        let mut up = Unpacker::new(&bytes[bytes.len() - 4..]);
        let num_restarts: u32 = up.unpack().map_err(|e: buffertk::Error| {
            CORRUPTION.click();
            Error::UnpackError {
                core: ErrorCore::default(),
                error: e.into(),
                context: "could not read last four bytes of block".to_string(),
            }
        })?;
        let num_restarts: usize = num_restarts as usize;
        // Footer size.
        // |tag 10|v64 of num bytes|packed num_restarts u32s|tag 11|fixed32 capstone|
        let capstone: usize = 1/*tag 11*/ + 4/*fixed32 capstone*/;
        let footer_body: usize = num_restarts * 4;
        let footer_head: usize = 1/*tag 10*/ + v64::from(footer_body).pack_sz();
        let restarts_idx = bytes.len() - capstone - footer_body;
        let restarts_boundary = restarts_idx - footer_head;
        // Reader.
        let block = Block {
            bytes,
            restarts_boundary,
            restarts_idx,
            num_restarts,
        };
        Ok(block)
    }

    /// Approximate size of the block, not including the struct itself.
    pub fn approximate_size(&self) -> usize {
        self.bytes.len()
    }

    /// Return a reference to the block's bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return a cursor over the block.
    pub fn cursor(&self) -> BlockCursor {
        BlockCursor::new(self.clone())
    }

    fn restart_point(&self, restart_idx: usize) -> usize {
        assert!(restart_idx < self.num_restarts);
        let mut restart: [u8; 4] = <[u8; 4]>::default();
        let bytes = &self.bytes;
        for i in 0..4 {
            restart[i] = bytes[self.restarts_idx + restart_idx * 4 + i];
        }
        u32::from_le_bytes(restart) as usize
    }

    fn restart_for_offset(&self, offset: usize) -> usize {
        let mut left: usize = 0usize;
        let mut right: usize = self.num_restarts - 1;

        // NOTE(rescrv):  This is not the same as the binary search below because we are looking
        // for incomplete ranges.  The value at i may cover a range [x, y) where restart[i + 1] = y.
        while left < right {
            // Pick a mid such that when left and right are adjacent, mid equal right.
            let mid = (left + right + 1) / 2;
            let value = self.restart_point(mid);
            match offset.cmp(&value) {
                Ordering::Less => {
                    // The offset is less than this restart point.  It cannot be contained within
                    // this restart.
                    right = mid - 1;
                }
                Ordering::Equal => {
                    // The offset exactly equals this restart point.  We're lucky that we can go
                    // home early.
                    left = mid;
                    right = mid;
                }
                Ordering::Greater => {
                    // The offset > value.  The best we can do is to move the left to the mid
                    // because it could still equal left.
                    left = mid;
                }
            }
        }

        left
    }
}

impl keyvalint::KeyValueLoad for Block {
    type Error = Error;
    type RangeScan<'a> = BoundsCursor<PruningCursor<BlockCursor, Error>, Error>;

    fn load(
        &self,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        *is_tombstone = false;
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
            if compare_bytes(kvr.key, key).is_eq() {
                *is_tombstone = kvr.value.is_none();
                Ok(kvr.value.as_ref().map(|v| v.to_vec()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        let pruning = PruningCursor::new(self.cursor(), timestamp)?;
        BoundsCursor::new(pruning, start_bound, end_bound)
    }
}

/////////////////////////////////////////// BlockBuilder ///////////////////////////////////////////

/// Build a block.
#[derive(Clone, Debug)]
pub struct BlockBuilder {
    options: BlockBuilderOptions,
    buffer: Vec<u8>,
    last_key: Vec<u8>,
    last_timestamp: u64,
    // Restart metadata.
    restarts: Vec<u32>,
    bytes_since_restart: u64,
    key_value_pairs_since_restart: u64,
}

impl BlockBuilder {
    /// Create a new block builder.
    pub fn new(options: BlockBuilderOptions) -> Self {
        let buffer = Vec::default();
        let restarts = vec![0];
        BlockBuilder {
            options,
            buffer,
            last_key: Vec::default(),
            last_timestamp: u64::max_value(),
            restarts,
            bytes_since_restart: 0,
            key_value_pairs_since_restart: 0,
        }
    }

    fn should_restart(&self) -> bool {
        self.options.bytes_restart_interval <= self.bytes_since_restart
            || self.options.key_value_pairs_restart_interval <= self.key_value_pairs_since_restart
    }

    fn compute_key_frag<'a>(&mut self, key: &'a [u8]) -> (usize, &'a [u8]) {
        let shared = if !self.should_restart() {
            let max_shared: usize = cmp::min(self.last_key.len(), key.len());
            let mut shared = 0;
            while shared < max_shared && key[shared] == self.last_key[shared] {
                shared += 1;
            }
            shared
        } else {
            // do a restart
            self.bytes_since_restart = 0;
            self.key_value_pairs_since_restart = 0;
            self.restarts.push(self.buffer.len() as u32);
            0
        };
        (shared, &key[shared..])
    }

    // TODO(rescrv):  Make sure to sort secondary by timestamp
    fn append(&mut self, be: KeyValueEntry<'_>) -> Result<(), Error> {
        // Update the last key.
        self.last_key.truncate(be.shared());
        self.last_key.extend_from_slice(be.key_frag());
        self.last_timestamp = be.timestamp();

        // Append to the vector.
        let pa = stack_pack(be);
        // This assert should be safe because our table size is limited to 1<<30 and be's pack size
        // should not exceed 3GiB.
        assert!(self.buffer.len() + pa.pack_sz() <= u32::max_value() as usize);
        pa.append_to_vec(&mut self.buffer);

        // Update the estimates for when we should do a restart.
        self.bytes_since_restart += pa.pack_sz() as u64;
        self.key_value_pairs_since_restart += 1;
        Ok(())
    }

    fn enforce_sort_order(&self, key: &[u8], timestamp: u64) -> Result<(), Error> {
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
}

impl Builder for BlockBuilder {
    type Sealed = Block;

    fn approximate_size(&self) -> usize {
        self.buffer.len() + 16 + self.restarts.len() * 4
    }

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Error> {
        check_key_len(key)?;
        check_value_len(value)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let (shared, key_frag) = self.compute_key_frag(key);
        let kvp = KeyValuePut {
            shared: shared as u64,
            key_frag,
            timestamp,
            value,
        };
        let be = KeyValueEntry::Put(kvp);
        self.append(be)
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), Error> {
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let (shared, key_frag) = self.compute_key_frag(key);
        let kvp = KeyValueDel {
            shared: shared as u64,
            key_frag,
            timestamp,
        };
        let be = KeyValueEntry::Del(kvp);
        self.append(be)
    }

    fn seal(self) -> Result<Block, Error> {
        // Append each restart.
        // NOTE(rescrv):  If this changes, change approximate_size above.
        let restarts = length_free(&self.restarts);
        let tag10: v64 = ((10 << 3) | 2).into();
        let tag11: v64 = ((11 << 3) | 5).into();
        let sz: v64 = restarts.pack_sz().into();
        let pa = stack_pack(tag10);
        let pa = pa.pack(sz);
        let pa = pa.pack(restarts);
        let pa = pa.pack(tag11);
        let pa = pa.pack(self.restarts.len() as u32);
        let mut contents = self.buffer;
        pa.append_to_vec(&mut contents);
        Block::new(contents)
    }
}

////////////////////////////////////////// CursorPosition //////////////////////////////////////////

#[derive(Clone, Debug)]
enum CursorPosition {
    First,
    Last,
    Positioned {
        restart_idx: usize,
        offset: usize,
        next_offset: usize,
        key: Vec<u8>,
        timestamp: u64,
    },
}

impl CursorPosition {
    fn is_positioned(&self) -> bool {
        match self {
            CursorPosition::First => false,
            CursorPosition::Last => false,
            CursorPosition::Positioned { .. } => true,
        }
    }
}

impl PartialEq for CursorPosition {
    fn eq(&self, rhs: &CursorPosition) -> bool {
        match (&self, &rhs) {
            (&CursorPosition::First, &CursorPosition::First) => true,
            (&CursorPosition::Last, &CursorPosition::Last) => true,
            (
                &CursorPosition::Positioned {
                    restart_idx: ri1,
                    offset: o1,
                    next_offset: no1,
                    key: ref k1,
                    timestamp: t1,
                },
                &CursorPosition::Positioned {
                    restart_idx: ri2,
                    offset: o2,
                    next_offset: no2,
                    key: ref k2,
                    timestamp: t2,
                },
            ) => ri1 == ri2 && o1 == o2 && no1 == no2 && k1 == k2 && t1 == t2,
            _ => false,
        }
    }
}

//////////////////////////////////////////// BlockCursor ///////////////////////////////////////////

/// A cursor over a block.
#[derive(Clone, Debug)]
pub struct BlockCursor {
    block: Block,
    position: CursorPosition,
}

impl BlockCursor {
    /// Create a new BlockCursor from the provided block.
    pub fn new(block: Block) -> Self {
        BlockCursor {
            block,
            position: CursorPosition::First,
        }
    }

    pub(crate) fn offset(&self) -> usize {
        match &self.position {
            CursorPosition::First => 0,
            CursorPosition::Last => self.block.restarts_boundary,
            CursorPosition::Positioned {
                restart_idx: _,
                offset,
                next_offset: _,
                key: _,
                timestamp: _,
            } => *offset,
        }
    }

    pub(crate) fn next_offset(&self) -> usize {
        match &self.position {
            CursorPosition::First => 0,
            CursorPosition::Last => self.block.restarts_boundary,
            CursorPosition::Positioned {
                restart_idx: _,
                offset: _,
                next_offset,
                key: _,
                timestamp: _,
            } => *next_offset,
        }
    }

    fn restart_idx(&self) -> usize {
        match &self.position {
            CursorPosition::First => 0,
            CursorPosition::Last => self.block.num_restarts,
            CursorPosition::Positioned {
                restart_idx,
                offset: _,
                next_offset: _,
                key: _,
                timestamp: _,
            } => *restart_idx,
        }
    }

    // Make self.position be of type CursorPosition::Positioned and fill in the fields.
    fn seek_restart(&mut self, restart_idx: usize) -> Result<Option<KeyRef>, Error> {
        if restart_idx >= self.block.num_restarts {
            LOGIC_ERROR.click();
            let err = Error::LogicError {
                core: ErrorCore::default(),
                context: "restart_idx exceeds num_restarts".to_string(),
            }
            .with_variable("restart_idx", restart_idx)
            .with_variable("num_restarts", self.block.num_restarts);
            return Err(err);
        }
        let offset = self.block.restart_point(restart_idx);
        if offset >= self.block.restarts_boundary {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "offset exceeds restarts_boundary".to_string(),
            }
            .with_variable("offset", offset)
            .with_variable("restarts_boundary", self.block.restarts_boundary);
            return Err(err);
        }

        // Extract the key from self.position.
        let prev_key = match self.position {
            CursorPosition::First => Vec::new(),
            CursorPosition::Last => Vec::new(),
            CursorPosition::Positioned {
                restart_idx: _,
                offset: _,
                next_offset: _,
                ref mut key,
                timestamp: _,
            } => {
                let mut ret = Vec::new();
                key.truncate(0);
                std::mem::swap(&mut ret, key);
                ret
            }
        };

        // Setup the position correctly and return what we see.
        self.position = BlockCursor::extract_key(&self.block, offset, prev_key)?;
        // Return the kvp for this offset.
        self.key_ref()
    }

    fn key_ref(&self) -> Result<Option<KeyRef>, Error> {
        match &self.position {
            CursorPosition::First => Ok(None),
            CursorPosition::Last => Ok(None),
            CursorPosition::Positioned {
                restart_idx: _,
                offset: _,
                next_offset: _,
                key,
                timestamp,
            } => Ok(Some(KeyRef {
                key,
                timestamp: *timestamp,
            })),
        }
    }

    fn extract_key(
        block: &Block,
        offset: usize,
        mut key: Vec<u8>,
    ) -> Result<CursorPosition, Error> {
        // Check for overrun.
        if offset >= block.restarts_boundary {
            return Ok(CursorPosition::Last);
        }
        // Parse the key-value pair.
        let bytes = &block.bytes;
        let mut up = Unpacker::new(&bytes[offset..block.restarts_boundary]);
        let be: KeyValueEntry = up.unpack().map_err(|e| {
            CORRUPTION.click();
            Error::UnpackError {
                core: ErrorCore::default(),
                error: e,
                context: "could not unpack key-value pair at offset".to_string(),
            }
            .with_variable("offset", offset)
        })?;
        let next_offset = block.restarts_boundary - up.remain().len();
        let restart_idx = block.restart_for_offset(offset);
        // Assemble the returnable cursor.
        key.truncate(be.shared());
        key.extend_from_slice(be.key_frag());
        Ok(CursorPosition::Positioned {
            restart_idx,
            offset,
            next_offset,
            key,
            timestamp: be.timestamp(),
        })
    }
}

impl Cursor for BlockCursor {
    type Error = Error;

    fn seek_to_first(&mut self) -> Result<(), Error> {
        self.position = CursorPosition::First;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Error> {
        self.position = CursorPosition::Last;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        // Make sure there are restarts.
        if self.block.num_restarts == 0 {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "a block with 0 restarts".to_string(),
            };
            return Err(err);
        }

        // Binary search to the correct restart point.
        let mut left: usize = 0usize;
        let mut right: usize = self.block.num_restarts - 1;
        while left < right {
            // When left and right are adjacent, it will seek to the right.
            let mid = left + (right - left + 1) / 2;
            let kvp = match self.seek_restart(mid)? {
                Some(x) => x,
                None => {
                    CORRUPTION.click();
                    let err = Error::Corruption {
                        core: ErrorCore::default(),
                        context: "restart point returned no key-value pair".to_string(),
                    }
                    .with_variable("restart_point", mid);
                    return Err(err);
                }
            };
            match compare_bytes(key, kvp.key) {
                Ordering::Less => {
                    // left     mid     right
                    // |--------|-------|
                    //       |
                    right = mid - 1;
                }
                Ordering::Equal => {
                    // left     mid     right
                    // |--------|-------|
                    //          |
                    right = mid - 1;
                }
                Ordering::Greater => {
                    // left     mid     right
                    // |--------|-------|
                    //           |
                    left = mid;
                }
            };
        }

        // Sanity check the outcome of the binary search.
        if left != right {
            CORRUPTION.click();
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "binary_search left != right".to_string(),
            }
            .with_variable("left", left)
            .with_variable("right", right);
            return Err(err);
        }

        // We position at the left restart point
        //
        // This may be redundant, but only about 50% of the time.  The complexity to get it right
        // all the time is not currently worth the savings.
        let kref = match self.seek_restart(left)? {
            Some(x) => x,
            None => {
                CORRUPTION.click();
                let err = Error::Corruption {
                    core: ErrorCore::default(),
                    context: "restart point returned no key-value pair".to_string(),
                }
                .with_variable("restart_point", left);
                return Err(err);
            }
        };

        // Scan until we find the key.
        let mut kref = Some(kref);
        while let Some(x) = kref {
            if compare_bytes(key, x.key).is_gt() {
                self.next()?;
                kref = self.key_ref()?;
            } else {
                break;
            }
        }

        Ok(())
    }

    fn prev(&mut self) -> Result<(), Error> {
        // We won't go past here.
        let target_next_offset = match self.position {
            CursorPosition::First => {
                return Ok(());
            }
            CursorPosition::Last => self.block.restarts_boundary,
            CursorPosition::Positioned {
                restart_idx: _,
                offset,
                next_offset: _,
                key: _,
                timestamp: _,
            } => offset,
        };

        // Boundary condition
        if target_next_offset == 0 {
            self.position = CursorPosition::First;
            return Ok(());
        }

        // Step to the correct restart point.  If this is the first value in a restart point, set
        // the restart_idx to the previous point, unless we are at the first restart point.
        let current_restart_idx = self.restart_idx();
        let restart_idx = if current_restart_idx >= self.block.num_restarts
            || target_next_offset <= self.block.restart_point(current_restart_idx)
        {
            if current_restart_idx == 0 {
                LOGIC_ERROR.click();
                let err = Error::LogicError {
                    core: ErrorCore::default(),
                    context: "tried taking the -1st restart_idx".to_string(),
                };
                return Err(err);
            }
            current_restart_idx - 1
        } else {
            current_restart_idx
        };

        // Seek and scan.
        self.seek_restart(restart_idx)
            .with_variable("restart_idx", restart_idx)?;
        while self.next_offset() < target_next_offset {
            self.next()
                .with_variable("target_next_offset", target_next_offset)?;
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), Error> {
        // We start with the first block.
        if let CursorPosition::First = self.position {
            self.seek_restart(0)?;
            return Ok(());
        }

        // We always return None for the next of Last.
        if let CursorPosition::Last = self.position {
            return Ok(());
        }

        // Hit up against the end, make it a Last.
        let offset = self.next_offset();
        if offset >= self.block.restarts_boundary {
            self.position = CursorPosition::Last;
            return Ok(());
        }

        // We are jumping to the next block, so use seek_restart.
        if self.restart_idx() + 1 < self.block.num_restarts
            && self.block.restart_point(self.restart_idx() + 1) <= offset
        {
            self.seek_restart(self.restart_idx() + 1)?;
            return Ok(());
        }

        // We are positioned.
        if !self.position.is_positioned() {
            LOGIC_ERROR.click();
            let err = Error::LogicError {
                core: ErrorCore::default(),
                context: "next was not positioned, but made it to here.".to_string(),
            };
            return Err(err);
        }

        // Extract the key from self.position.
        let prev_key = match self.position {
            CursorPosition::First => Vec::new(),
            CursorPosition::Last => Vec::new(),
            CursorPosition::Positioned {
                restart_idx: _,
                offset: _,
                next_offset: _,
                ref mut key,
                timestamp: _,
            } => std::mem::take(key),
        };

        // Setup the position correctly and return what we see.
        self.position = BlockCursor::extract_key(&self.block, offset, prev_key)
            .with_variable("offset", offset)?;
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        match &self.position {
            CursorPosition::First => None,
            CursorPosition::Last => None,
            CursorPosition::Positioned {
                restart_idx: _,
                offset: _,
                next_offset: _,
                key,
                timestamp,
            } => Some(KeyRef {
                key,
                timestamp: *timestamp,
            }),
        }
    }

    fn value(&self) -> Option<&[u8]> {
        match &self.position {
            CursorPosition::First => None,
            CursorPosition::Last => None,
            CursorPosition::Positioned {
                restart_idx: _,
                offset,
                next_offset: _,
                key: _,
                timestamp: _,
            } => {
                // Parse the value from the block entry.
                let bytes = &self.block.bytes;
                let mut up = Unpacker::new(&bytes[*offset..self.block.restarts_boundary]);
                let be: KeyValueEntry = up
                    .unpack()
                    .expect("already parsed this block with extract_key; corruption");
                be.value()
            }
        }
    }
}

impl From<Block> for BlockCursor {
    fn from(table: Block) -> Self {
        Self::new(table)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_empty_block() {
        let builder = BlockBuilder::new(BlockBuilderOptions::default());
        let block = builder.seal().unwrap();
        let got: &[u8] = &block.bytes;
        let exp: &[u8] = &[82, 4, 0, 0, 0, 0, 93, 1, 0, 0, 0];
        assert_eq!(exp, got);
        assert_eq!(11, got.len());
    }

    #[test]
    fn build_single_item_block() {
        let mut builder = BlockBuilder::new(BlockBuilderOptions::default());
        builder
            .put("key".as_bytes(), 0xc0ffee, "value".as_bytes())
            .unwrap();
        let block = builder.seal().unwrap();
        let got: &[u8] = &block.bytes;
        let exp = &[
            66, /*8*/
            19, /*sz*/
            8,  /*1*/
            0,  /*zero*/
            18, /*2*/
            3,  /*sz*/
            107, 101, 121, 24, /*3*/
            /*varint(0xc0ffee):*/ 238, 255, 131, 6, 34, /*4*/
            5,  /*sz*/
            118, 97, 108, 117, 101, // restarts
            82,  /*10*/
            4,   /*sz*/
            0, 0, 0, 0,  // num_restarts
            93, /*11*/
            1, 0, 0, 0,
        ];
        assert_eq!(exp, got);
    }

    #[test]
    fn build_prefix_compression() {
        let mut builder = BlockBuilder::new(BlockBuilderOptions::default());
        builder
            .put("key1".as_bytes(), 0xc0ffee, "value1".as_bytes())
            .unwrap();
        builder
            .put("key2".as_bytes(), 0xc0ffee, "value2".as_bytes())
            .unwrap();
        let block = builder.seal().unwrap();
        let got: &[u8] = &block.bytes;
        let exp = &[
            // first record
            66, /*8*/
            21, /*sz*/
            8,  /*1*/
            0, 18, /*2*/
            4,  /*sz*/
            107, 101, 121, 49, 24, /*3*/
            /*varint(0xc0ffee)*/ 238, 255, 131, 6, 34, /*4*/
            6,  /*sz*/
            118, 97, 108, 117, 101, 49, // second record
            66, /*8*/
            18, /*sz*/
            8,  /*1*/
            3, 18, /*2*/
            1,  /*sz*/
            50, 24, /*3*/
            /*varint(0xc0ffee)*/ 238, 255, 131, 6, 34, /*4*/
            6,  /*sz*/
            118, 97, 108, 117, 101, 50, // restarts
            82, /*10*/
            4,  /*sz*/
            0, 0, 0, 0, 93, /*11*/
            1, 0, 0, 0,
        ];
        assert_eq!(exp, got);
    }

    #[test]
    fn load_restart_points() {
        let block_bytes = &[
            // first record
            66, /*8*/
            21, /*sz*/
            8,  /*1*/
            0, 18, /*2*/
            4,  /*sz*/
            107, 101, 121, 49, 24, /*3*/
            /*varint(0xc0ffee)*/ 238, 255, 131, 6, 34, /*4*/
            6,  /*sz*/
            118, 97, 108, 117, 101, 49, // second record
            66, /*8*/
            21, /*sz*/
            8,  /*1*/
            0, 18, /*2*/
            4,  /*sz*/
            107, 101, 121, 50, 24, /*3*/
            /*varint(0xc0ffee)*/ 238, 255, 131, 6, 34, /*4*/
            6,  /*sz*/
            118, 97, 108, 117, 101, 50, // restarts
            82, /*10*/
            8,  /*sz*/
            0, 0, 0, 0, 22, 0, 0, 0, 93, /*11*/
            2, 0, 0, 0,
        ];
        let block = Block::new(block_bytes.to_vec()).unwrap();
        assert_eq!(2, block.num_restarts);
        assert_eq!(0, block.restart_point(0));
        assert_eq!(22, block.restart_point(1));
    }

    #[test]
    fn corruption_bug_gone() {
        let key = &[107, 65, 118, 119, 82, 109, 53, 69];
        let timestamp = 4092481979873166344;
        let value = &[120, 100, 81, 80, 75, 79, 121, 90];
        let mut builder = BlockBuilder::new(BlockBuilderOptions::default());
        builder.put(key, timestamp, value).unwrap();
        let block = builder.seal().unwrap();
        let exp = &[
            // record
            66, /*8*/
            32, /*sz*/
            8,  /*1*/
            0, 18, /*2*/
            8,  /*sz*/
            107, 65, 118, 119, 82, 109, 53, 69, 24, /*3*/
            /*varint*/ 136, 136, 156, 160, 216, 213, 218, 229, 56, 34, /*4*/
            8,  /*sz*/
            120, 100, 81, 80, 75, 79, 121, 90, // restarts
            82, /*10*/
            4,  /*sz*/
            0, 0, 0, 0, 93, /*11*/
            1, 0, 0, 0,
        ];
        let got: &[u8] = &block.bytes;
        assert_eq!(exp, got);

        let mut cursor = block.cursor();
        cursor.seek(&[106, 113, 67, 73, 122, 73, 98, 85]).unwrap();
    }

    #[test]
    fn seek_bug_gone() {
        let key = "kAvwRm5E";
        let timestamp = 4092481979873166344;
        let value = "xdQPKOyZwQUykR8i";

        let mut block = BlockBuilder::new(BlockBuilderOptions::default());
        block
            .put(key.as_bytes(), timestamp, value.as_bytes())
            .unwrap();
        let block = block.seal().unwrap();

        let mut cursor = block.cursor();
        let target = "jqCIzIbU";
        cursor.seek(target.as_bytes()).unwrap();
        let key: Vec<u8> = key.into();
        let kvp = cursor.key_value().unwrap();
        assert_eq!(&key, kvp.key);
        assert_eq!(timestamp, kvp.timestamp);
    }

    #[test]
    fn cursor_equals() {
        let lhs = CursorPosition::First;
        let rhs = CursorPosition::First;
        assert_eq!(lhs, rhs);

        let lhs = CursorPosition::Last;
        let rhs = CursorPosition::Last;
        assert_eq!(lhs, rhs);

        let lhs = CursorPosition::Positioned {
            restart_idx: 0,
            offset: 0,
            next_offset: 19,
            key: "E".into(),
            timestamp: 17563921251225492277,
        };
        let rhs = CursorPosition::Positioned {
            restart_idx: 0,
            offset: 0,
            next_offset: 19,
            key: "E".into(),
            timestamp: 17563921251225492277,
        };
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn extract_key() {
        let bytes = &[
            // record
            66, /*8*/
            18, /*sz*/
            8,  /*1*/
            0, 18, /*2*/
            1,  /*sz*/
            69, 24, /*3*/
            /*varint*/ 181, 182, 235, 145, 160, 170, 229, 223, 243, 1, 34, /*4*/
            0,  /*sz*/
            // record
            66, /*8*/
            17, /*sz*/
            8,  /*1*/
            0, 18, /*2*/
            1,  /*sz*/
            107, 24, /*3*/
            /*varint*/ 136, 136, 156, 160, 216, 213, 218, 229, 56, 34, /*4*/
            0,  /*sz*/
            // restarts
            82, /*10*/
            4,  /*sz*/
            0, 0, 0, 0, 93, /*11*/
            1, 0, 0, 0,
        ];
        let block = Block::new(bytes.to_vec()).unwrap();

        let exp = CursorPosition::Positioned {
            restart_idx: 0,
            offset: 0,
            next_offset: 20,
            key: "E".into(),
            timestamp: 17563921251225492277,
        };
        let got = BlockCursor::extract_key(&block, 0, Vec::new()).unwrap();
        assert_eq!(exp, got);

        let exp = CursorPosition::Positioned {
            restart_idx: 0,
            offset: 20,
            next_offset: 39,
            key: "k".into(),
            timestamp: 4092481979873166344,
        };
        let got = BlockCursor::extract_key(&block, 20, Vec::new()).unwrap();
        assert_eq!(exp, got);

        let exp = CursorPosition::Last;
        let got = BlockCursor::extract_key(&block, 39, Vec::new()).unwrap();
        assert_eq!(exp, got);
    }
}
