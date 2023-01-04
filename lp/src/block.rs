use std::cmp;
use std::cmp::Ordering;
use std::rc::Rc;

use buffertk::{length_free, stack_pack, v64, Packable, Unpacker};

use prototk::field_types::*;
use prototk_derive::Message;

use zerror::{ZError, ZErrorResult};

use super::{
    LOGIC_ERROR, CORRUPTION, check_key_len, check_table_size, check_value_len, compare_bytes, compare_key,
    Buffer, Builder, Cursor, Error, KeyRef, KeyValueRef,
};

//////////////////////////////////////// BlockBuilderOptions ///////////////////////////////////////

#[derive(Clone, Debug)]
pub struct BlockBuilderOptions {
    bytes_restart_interval: u64,
    key_value_pairs_restart_interval: u64,
}

impl BlockBuilderOptions {
    pub fn bytes_restart_interval(mut self, bytes_restart_interval: u32) -> Self {
        self.bytes_restart_interval = bytes_restart_interval as u64;
        self
    }

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

//////////////////////////////////////////// BlockEntry ////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
enum BlockEntry<'a> {
    #[prototk(8, message)]
    KeyValuePut(KeyValuePut<'a>),
    #[prototk(9, message)]
    KeyValueDel(KeyValueDel<'a>),
}

impl<'a> BlockEntry<'a> {
    fn shared(&self) -> usize {
        match self {
            BlockEntry::KeyValuePut(x) => x.shared as usize,
            BlockEntry::KeyValueDel(x) => x.shared as usize,
        }
    }

    fn key_frag(&self) -> &'a [u8] {
        match self {
            BlockEntry::KeyValuePut(x) => x.key_frag,
            BlockEntry::KeyValueDel(x) => x.key_frag,
        }
    }

    fn timestamp(&self) -> u64 {
        match self {
            BlockEntry::KeyValuePut(x) => x.timestamp,
            BlockEntry::KeyValueDel(x) => x.timestamp,
        }
    }

    fn value(&self) -> Option<&'a [u8]> {
        match self {
            BlockEntry::KeyValuePut(x) => Some(x.value),
            BlockEntry::KeyValueDel(_) => None,
        }
    }
}

impl<'a> Default for BlockEntry<'a> {
    fn default() -> Self {
        Self::KeyValuePut(KeyValuePut::default())
    }
}

/////////////////////////////////////////////// Block //////////////////////////////////////////////

#[derive(Clone)]
pub struct Block {
    // The raw bytes built by a builder or loaded off disk.
    bytes: Rc<Buffer>,

    // The restart intervals.  restarts_boundary points to the first restart point.
    restarts_boundary: usize,
    restarts_idx: usize,
    num_restarts: usize,
}

impl Block {
    pub fn new(bytes: Buffer) -> Result<Self, ZError<Error>> {
        // Load num_restarts.
        let bytes = Rc::new(bytes);
        if bytes.len() < 4 {
            // This is impossible.  A block must end in a u32 that indicates how many restarts
            // there are.
            return Err(ZError::new(Error::BlockTooSmall {
                length: bytes.len(),
                required: 4,
            }));
        }
        let mut up = Unpacker::new(&bytes.as_bytes()[bytes.len() - 4..]);
        let num_restarts: u32 = up.unpack().map_err(|e: buffertk::Error| {
            CORRUPTION.click();
            ZError::new(Error::UnpackError {
                error: e.into(),
                context: "could not read last four bytes of block".to_string(),
            })
        })?;
        let num_restarts: usize = num_restarts as usize;
        // Footer size.
        // |tag 10|v64 of num bytes|packed num_restarts u32s|tag 11|fixed32 capstone|
        let capstone: usize = 1/*tag 11*/ + 4/*fixed32 capstone*/;
        let footer_body: usize = num_restarts as usize * 4;
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

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_bytes()
    }

    pub fn cursor(&self) -> BlockCursor {
        BlockCursor::new(self.clone())
    }

    fn restart_point(&self, restart_idx: usize) -> usize {
        assert!(restart_idx < self.num_restarts as usize);
        let mut restart: [u8; 4] = <[u8; 4]>::default();
        let bytes = self.bytes.as_bytes();
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

/////////////////////////////////////////// BlockBuilder ///////////////////////////////////////////

// Build a block.
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
    fn append<'a>(&mut self, be: BlockEntry<'a>) -> Result<(), ZError<Error>> {
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

    fn enforce_sort_order(&self, key: &[u8], timestamp: u64) -> Result<(), ZError<Error>> {
        if compare_key(&self.last_key, self.last_timestamp, key, timestamp) != Ordering::Less {
            Err(ZError::new(Error::SortOrder {
                last_key: self.last_key.clone(),
                last_timestamp: self.last_timestamp,
                new_key: key.to_vec(),
                new_timestamp: timestamp,
            }))
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

    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), ZError<Error>> {
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
        let be = BlockEntry::KeyValuePut(kvp);
        self.append(be)
    }

    fn del(&mut self, key: &[u8], timestamp: u64) -> Result<(), ZError<Error>> {
        check_key_len(key)?;
        check_table_size(self.approximate_size())?;
        self.enforce_sort_order(key, timestamp)?;
        let (shared, key_frag) = self.compute_key_frag(key);
        let kvp = KeyValueDel {
            shared: shared as u64,
            key_frag,
            timestamp,
        };
        let be = BlockEntry::KeyValueDel(kvp);
        self.append(be)
    }

    fn seal(self) -> Result<Block, ZError<Error>> {
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
        Block::new(contents.into())
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

pub struct BlockCursor {
    block: Block,
    position: CursorPosition,
}

impl BlockCursor {
    pub fn new(block: Block) -> Self {
        BlockCursor {
            block,
            position: CursorPosition::First,
        }
    }

    fn next_offset(&self) -> usize {
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
    fn seek_restart(&mut self, restart_idx: usize) -> Result<Option<KeyRef>, ZError<Error>> {
        if restart_idx >= self.block.num_restarts {
            LOGIC_ERROR.click();
            let zerr = ZError::new(Error::LogicError {
                context: "restart_idx exceeds num_restarts".to_string(),
            })
            .with_context::<uint64>("restart_idx", 1, restart_idx as u64)
            .with_context::<uint64>("num_restarts", 2, self.block.num_restarts as u64);
            return Err(zerr);
        }
        let offset = self.block.restart_point(restart_idx);
        if offset >= self.block.restarts_boundary {
            CORRUPTION.click();
            let zerr = ZError::new(Error::Corruption {
                context: "offset exceeds restarts_boundary".to_string(),
            })
            .with_context::<uint64>("offset", 1, offset as u64)
            .with_context::<uint64>("restarts_boundary", 2, self.block.restarts_boundary as u64);
            return Err(zerr);
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

    fn key_ref(&self) -> Result<Option<KeyRef>, ZError<Error>> {
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
                key: key,
                timestamp: *timestamp,
            })),
        }
    }

    fn extract_key(
        block: &Block,
        offset: usize,
        mut key: Vec<u8>,
    ) -> Result<CursorPosition, ZError<Error>> {
        // Check for overrun.
        if offset >= block.restarts_boundary {
            return Ok(CursorPosition::Last);
        }
        // Parse the key-value pair.
        let bytes = block.bytes.as_bytes();
        let mut up = Unpacker::new(&bytes[offset..block.restarts_boundary]);
        let be: BlockEntry = up.unpack().map_err(|e| {
            CORRUPTION.click();
            ZError::new(Error::UnpackError {
                error: e,
                context: "could not unpack key-value pair at offset".to_string(),
            })
            .with_context::<uint64>("offset", 1, offset as u64)
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
    fn seek_to_first(&mut self) -> Result<(), ZError<Error>> {
        self.position = CursorPosition::First;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), ZError<Error>> {
        self.position = CursorPosition::Last;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), ZError<Error>> {
        // Make sure there are restarts.
        if self.block.num_restarts == 0 {
            CORRUPTION.click();
            let zerr = ZError::new(Error::Corruption {
                context: "a block with 0 restarts".to_string(),
            });
            return Err(zerr);
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
                    let zerr = ZError::new(Error::Corruption {
                        context: "restart point returned no key-value pair".to_string(),
                    })
                    .with_context::<uint64>("restart_point", 1, mid as u64);
                    return Err(zerr);
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
            let zerr = ZError::new(Error::Corruption {
                context: "binary_search left != right".to_string(),
            })
            .with_context::<uint64>("left", 1, left as u64)
            .with_context::<uint64>("right", 2, right as u64);
            return Err(zerr)
        }

        // We position at the left restart point
        //
        // This may be redundant, but only about 50% of the time.  The complexity to get it right
        // all the time is not currently worth the savings.
        let kref = match self.seek_restart(left)? {
            Some(x) => x,
            None => {
                CORRUPTION.click();
                let zerr = ZError::new(Error::Corruption {
                    context: "restart point returned no key-value pair".to_string(),
                })
                .with_context::<uint64>("restart_point", 1, left as u64);
                return Err(zerr);
            }
        };

        // Check for the case where all keys are bigger.
        if compare_bytes(key, kref.key).is_lt() {
            self.position = CursorPosition::First;
            return Ok(());
        }

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

        // Adjust the next_offset for prev/next.  prev will operate off offset, which is positioned
        // accordingly.  next will operate off next_offset.  Adjust it downward to offset so the
        // next key returned will be the key we seek'ed to.
        match &mut self.position {
            CursorPosition::First => {}
            CursorPosition::Last => {}
            CursorPosition::Positioned {
                restart_idx: _,
                offset,
                next_offset,
                key: _,
                timestamp: _,
            } => {
                *next_offset = *offset;
            }
        }

        Ok(())
    }

    fn prev(&mut self) -> Result<(), ZError<Error>> {
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
        if target_next_offset <= 0 {
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
                let zerr = ZError::new(Error::LogicError {
                    context: "tried taking the -1st restart_idx".to_string(),
                });
                return Err(zerr);
            }
            current_restart_idx - 1
        } else {
            current_restart_idx
        };

        // Seek and scan.
        self.seek_restart(restart_idx)
            .with_context::<uint64>("restart_idx", 1, restart_idx as u64)?;
        while self.next_offset() < target_next_offset {
            self.next()
                .with_context::<uint64>("target_next_offset", 1, target_next_offset as u64)?;
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), ZError<Error>> {
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
            let zerr = ZError::new(Error::LogicError {
                context: "next was not positioned, but made it to here.".to_string(),
            });
            return Err(zerr);
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
                std::mem::swap(&mut ret, key);
                ret
            }
        };

        // Setup the position correctly and return what we see.
        self.position = BlockCursor::extract_key(&self.block, offset, prev_key)
            .with_context::<uint64>("offset", 3, offset as u64)?;
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
                key: &key,
                timestamp: *timestamp,
            }),
        }
    }

    fn value(&self) -> Option<KeyValueRef> {
        match &self.position {
            CursorPosition::First => None,
            CursorPosition::Last => None,
            CursorPosition::Positioned {
                restart_idx: _,
                offset,
                next_offset: _,
                key,
                timestamp,
            } => {
                // Parse the value from the block entry.
                let bytes = self.block.bytes.as_bytes();
                let mut up = Unpacker::new(&bytes[*offset..self.block.restarts_boundary]);
                let be: BlockEntry = up
                    .unpack()
                    .expect("already parsed this block with extract_key; corruption");
                Some(KeyValueRef {
                    key: &key,
                    timestamp: *timestamp,
                    value: be.value(),
                })
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
        let got = block.bytes.as_bytes();
        let exp = &[82, 4, 0, 0, 0, 0, 93, 1, 0, 0, 0];
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
        let got = block.bytes.as_bytes();
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
        let got = block.bytes.as_bytes();
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
        let block = Block::new(block_bytes.to_vec().try_into().unwrap()).unwrap();
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
        let got = block.bytes.as_bytes();
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
        let key: Buffer = key.into();
        cursor.next().unwrap();
        let kvp = cursor.value().unwrap();
        assert_eq!(key.as_bytes(), kvp.key);
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
        let block = Block::new(bytes.to_vec().try_into().unwrap()).unwrap();

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

#[cfg(test)]
mod guacamole {
    use super::super::KeyValueRef;
    use super::*;

    #[test]
    fn human_guacamole_1() {
        // --num-keys 2
        // --key-bytes 1
        // --value-bytes 0
        // --num-seeks 1000
        // --seek-distance 10
        let builder_opts = BlockBuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = BlockBuilder::new(builder_opts);
        builder
            .put("E".as_bytes(), 17563921251225492277, "".as_bytes())
            .unwrap();
        builder
            .put("k".as_bytes(), 4092481979873166344, "".as_bytes())
            .unwrap();

        let block = builder.seal().unwrap();
        let exp = [
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
        let bytes: &[u8] = block.bytes.as_bytes();
        assert_eq!(exp, bytes);

        let mut cursor = block.cursor();
        match cursor.position {
            CursorPosition::First => {}
            _ => {
                panic!("cursor should always init to head: {:?}", cursor.position)
            }
        };
        cursor.seek("t".as_bytes()).unwrap();
        match cursor.position {
            CursorPosition::Last => {}
            _ => {
                panic!("cursor should seek to the end: {:?}", cursor.position)
            }
        };
        cursor.next().unwrap();
        let got = cursor.value();
        assert_eq!(None, got);
    }

    #[test]
    fn human_guacamole_2() {
        // --num-keys 10
        // --key-bytes 1
        // --value-bytes 64
        // --num-seeks 1
        // --seek-distance 4
        let builder_opts = BlockBuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = BlockBuilder::new(builder_opts);
        builder
            .put(
                "4".as_bytes(),
                5220327133503220768,
                "TFJaKOq4itZUjZ6zLYRQAtaYQJ2KOABpaX5Jxr07mN9NgTFUN70JdcuwGubnsBSV".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "A".as_bytes(),
                2365635627947495809,
                "JMbW18opQPCC6OsP5XSbF5bs9LWzNwSjS2uQKhkDv7rATMznKwv6yA5jWq0Ya77j".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "E".as_bytes(),
                17563921251225492277,
                "ZVaW3VAlMCSMzUF7lOFVun1pObMORRWajFd0gvzfK1Qwtyp0L8GnEfN1TBoDgG6v".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "I".as_bytes(),
                3844377046565620216,
                "0lfqYezeQ1mM8HYtpTNLVB4XQi8KAb2ouxCTLHjMTzGxBFaHuVVY1Osd23MrzSA6".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "J".as_bytes(),
                14848435744026832213,
                "RH53KxwpLPbrUJat64bFvDMqLXVEXfxwL1LAfVBVzcbsEd5QaIzUyPfhuIOvcUiw".as_bytes(),
            )
            .unwrap();
        builder.del("U".as_bytes(), 8329339752768468916).unwrap();
        builder
            .put(
                "g".as_bytes(),
                10374159306796994843,
                "SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "k".as_bytes(),
                4092481979873166344,
                "xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "t".as_bytes(),
                7790837488841419319,
                "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "v".as_bytes(),
                2133827469768204743,
                "5NV1fDTU6IBuTs5qP7mdDRrBlMCUlsVzXrk8dbMTjhrzdEaLtOSuC5sL3401yvrs".as_bytes(),
            )
            .unwrap();
        let block = builder.seal().unwrap();
        // Top of loop seeks to: Key { key: "d" }
        let mut cursor = block.cursor();
        cursor.seek("d".as_bytes()).unwrap();
        // Next to g
        cursor.next().unwrap();
        let got = cursor.value().unwrap();
        assert_eq!("g".as_bytes(), got.key);
        assert_eq!(10374159306796994843, got.timestamp);
        assert_eq!(
            Some("SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes()),
            got.value
        );
        assert_eq!(
            CursorPosition::Positioned {
                restart_idx: 0,
                offset: 434,
                next_offset: 518,
                key: "g".into(),
                timestamp: 10374159306796994843,
            },
            cursor.position
        );
        // Next to k
        cursor.next().unwrap();
        let got = cursor.value().unwrap();
        assert_eq!("k".as_bytes(), got.key);
        assert_eq!(4092481979873166344, got.timestamp);
        assert_eq!(
            CursorPosition::Positioned {
                restart_idx: 1,
                offset: 518,
                next_offset: 601,
                key: "k".into(),
                timestamp: 4092481979873166344,
            },
            cursor.position
        );
        // Next to t
        cursor.next().unwrap();
        let got = cursor.value().unwrap();
        let exp = KeyValueRef {
            key: "t".as_bytes(),
            timestamp: 7790837488841419319,
            value: Some(
                "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes(),
            ),
        };
        assert_eq!(exp, got);
        assert_eq!("t".as_bytes(), got.key);
        assert_eq!(7790837488841419319, got.timestamp);
        assert_eq!(
            Some("mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes()),
            got.value
        );
        assert_eq!(
            CursorPosition::Positioned {
                restart_idx: 1,
                offset: 601,
                next_offset: 684,
                key: "t".into(),
                timestamp: 7790837488841419319,
            },
            cursor.position
        );
    }

    #[test]
    fn guacamole_2() {
        // --num-keys 10
        // --key-bytes 1
        // --value-bytes 64
        // --num-seeks 1
        // --seek-distance 4
        let builder_opts = BlockBuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = BlockBuilder::new(builder_opts);
        builder
            .put(
                "4".as_bytes(),
                5220327133503220768,
                "TFJaKOq4itZUjZ6zLYRQAtaYQJ2KOABpaX5Jxr07mN9NgTFUN70JdcuwGubnsBSV".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "A".as_bytes(),
                2365635627947495809,
                "JMbW18opQPCC6OsP5XSbF5bs9LWzNwSjS2uQKhkDv7rATMznKwv6yA5jWq0Ya77j".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "E".as_bytes(),
                17563921251225492277,
                "ZVaW3VAlMCSMzUF7lOFVun1pObMORRWajFd0gvzfK1Qwtyp0L8GnEfN1TBoDgG6v".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "I".as_bytes(),
                3844377046565620216,
                "0lfqYezeQ1mM8HYtpTNLVB4XQi8KAb2ouxCTLHjMTzGxBFaHuVVY1Osd23MrzSA6".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "J".as_bytes(),
                14848435744026832213,
                "RH53KxwpLPbrUJat64bFvDMqLXVEXfxwL1LAfVBVzcbsEd5QaIzUyPfhuIOvcUiw".as_bytes(),
            )
            .unwrap();
        builder.del("U".as_bytes(), 8329339752768468916).unwrap();
        builder
            .put(
                "g".as_bytes(),
                10374159306796994843,
                "SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "k".as_bytes(),
                4092481979873166344,
                "xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "t".as_bytes(),
                7790837488841419319,
                "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "v".as_bytes(),
                2133827469768204743,
                "5NV1fDTU6IBuTs5qP7mdDRrBlMCUlsVzXrk8dbMTjhrzdEaLtOSuC5sL3401yvrs".as_bytes(),
            )
            .unwrap();
        let block = builder.seal().unwrap();
        // Top of loop seeks to: Key { key: "d" }
        let mut cursor = block.cursor();
        cursor.seek("d".as_bytes()).unwrap();
        cursor.next().unwrap();
        cursor.next().unwrap();
        cursor.next().unwrap();
        let got = cursor.value().unwrap();
        let exp = KeyValueRef {
            key: "t".as_bytes(),
            timestamp: 7790837488841419319,
            value: Some(
                "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes(),
            ),
        };
        assert_eq!(exp, got);
    }

    #[test]
    fn human_guacamole_3() {
        // --num-keys 10
        // --key-bytes 1
        // --value-bytes 64
        // --num-seeks 10
        // --seek-distance 1
        let builder_opts = BlockBuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = BlockBuilder::new(builder_opts);
        builder
            .put(
                "4".as_bytes(),
                5220327133503220768,
                "TFJaKOq4itZUjZ6zLYRQAtaYQJ2KOABpaX5Jxr07mN9NgTFUN70JdcuwGubnsBSV".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "A".as_bytes(),
                2365635627947495809,
                "JMbW18opQPCC6OsP5XSbF5bs9LWzNwSjS2uQKhkDv7rATMznKwv6yA5jWq0Ya77j".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "E".as_bytes(),
                17563921251225492277,
                "ZVaW3VAlMCSMzUF7lOFVun1pObMORRWajFd0gvzfK1Qwtyp0L8GnEfN1TBoDgG6v".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "I".as_bytes(),
                3844377046565620216,
                "0lfqYezeQ1mM8HYtpTNLVB4XQi8KAb2ouxCTLHjMTzGxBFaHuVVY1Osd23MrzSA6".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "J".as_bytes(),
                14848435744026832213,
                "RH53KxwpLPbrUJat64bFvDMqLXVEXfxwL1LAfVBVzcbsEd5QaIzUyPfhuIOvcUiw".as_bytes(),
            )
            .unwrap();
        builder.del("U".as_bytes(), 8329339752768468916).unwrap();
        builder
            .put(
                "g".as_bytes(),
                10374159306796994843,
                "SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "k".as_bytes(),
                4092481979873166344,
                "xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "t".as_bytes(),
                7790837488841419319,
                "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes(),
            )
            .unwrap();
        builder
            .put(
                "v".as_bytes(),
                2133827469768204743,
                "5NV1fDTU6IBuTs5qP7mdDRrBlMCUlsVzXrk8dbMTjhrzdEaLtOSuC5sL3401yvrs".as_bytes(),
            )
            .unwrap();
        let block = builder.seal().unwrap();
        // Top of loop seeks to: Key { key: "u" }
        let mut cursor = block.cursor();
        cursor.seek("u".as_bytes()).unwrap();
    }

    #[test]
    fn guacamole_4() {
        // --num-keys 100
        // --key-bytes 1
        // --value-bytes 0
        // --num-seeks 1
        // --seek-distance 4
        let builder_opts = BlockBuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = BlockBuilder::new(builder_opts);
        builder
            .put("0".as_bytes(), 9697512111035884403, "".as_bytes())
            .unwrap();
        builder
            .put("1".as_bytes(), 3798246989967619197, "".as_bytes())
            .unwrap();
        builder
            .put("2".as_bytes(), 10342091538431028726, "".as_bytes())
            .unwrap();
        builder
            .put("3".as_bytes(), 15157365073906098091, "".as_bytes())
            .unwrap();
        builder
            .put("3".as_bytes(), 9466660179799601223, "".as_bytes())
            .unwrap();
        builder
            .put("3".as_bytes(), 5028655377053437110, "".as_bytes())
            .unwrap();
        builder
            .put("4".as_bytes(), 16805872069322243742, "".as_bytes())
            .unwrap();
        builder
            .put("4".as_bytes(), 16112959034514062976, "".as_bytes())
            .unwrap();
        builder
            .put("4".as_bytes(), 7876299547345770848, "".as_bytes())
            .unwrap();
        builder
            .put("4".as_bytes(), 5220327133503220768, "".as_bytes())
            .unwrap();
        builder
            .put("7".as_bytes(), 14395010029865413065, "".as_bytes())
            .unwrap();
        builder
            .put("8".as_bytes(), 17618669414409465042, "".as_bytes())
            .unwrap();
        builder
            .put("8".as_bytes(), 13191224295862555992, "".as_bytes())
            .unwrap();
        builder
            .put("8".as_bytes(), 5084626311153408505, "".as_bytes())
            .unwrap();
        builder
            .put("9".as_bytes(), 12995477672441385068, "".as_bytes())
            .unwrap();
        builder
            .put("A".as_bytes(), 9605838007579610207, "".as_bytes())
            .unwrap();
        builder
            .put("A".as_bytes(), 2365635627947495809, "".as_bytes())
            .unwrap();
        builder
            .put("A".as_bytes(), 1952263260996816483, "".as_bytes())
            .unwrap();
        builder
            .put("B".as_bytes(), 10126582942351468573, "".as_bytes())
            .unwrap();
        builder
            .put("C".as_bytes(), 16217491379957293402, "".as_bytes())
            .unwrap();
        builder
            .put("C".as_bytes(), 1973107251517101738, "".as_bytes())
            .unwrap();
        builder
            .put("E".as_bytes(), 17563921251225492277, "".as_bytes())
            .unwrap();
        builder
            .put("F".as_bytes(), 7744344282933500472, "".as_bytes())
            .unwrap();
        builder
            .put("F".as_bytes(), 7572175103299679188, "".as_bytes())
            .unwrap();
        builder
            .put("G".as_bytes(), 3562951228830167005, "".as_bytes())
            .unwrap();
        builder
            .put("H".as_bytes(), 10415469497441400582, "".as_bytes())
            .unwrap();
        builder
            .put("I".as_bytes(), 3844377046565620216, "".as_bytes())
            .unwrap();
        builder
            .put("J".as_bytes(), 17476236525666259675, "".as_bytes())
            .unwrap();
        builder
            .put("J".as_bytes(), 14848435744026832213, "".as_bytes())
            .unwrap();
        builder
            .put("K".as_bytes(), 5137225721270789888, "".as_bytes())
            .unwrap();
        builder
            .put("K".as_bytes(), 4825960407565437069, "".as_bytes())
            .unwrap();
        builder
            .put("L".as_bytes(), 15335622082534854763, "".as_bytes())
            .unwrap();
        builder
            .put("L".as_bytes(), 7211574025721472487, "".as_bytes())
            .unwrap();
        builder
            .put("M".as_bytes(), 485375931245920424, "".as_bytes())
            .unwrap();
        builder
            .put("O".as_bytes(), 6226508136092163051, "".as_bytes())
            .unwrap();
        builder
            .put("P".as_bytes(), 11429503906557966656, "".as_bytes())
            .unwrap();
        builder
            .put("P".as_bytes(), 6890969690330950371, "".as_bytes())
            .unwrap();
        builder
            .put("P".as_bytes(), 1488139426474409410, "".as_bytes())
            .unwrap();
        builder
            .put("P".as_bytes(), 418483046145178590, "".as_bytes())
            .unwrap();
        builder
            .put("R".as_bytes(), 13695467658803848996, "".as_bytes())
            .unwrap();
        builder
            .put("R".as_bytes(), 9039056961022621355, "".as_bytes())
            .unwrap();
        builder
            .put("T".as_bytes(), 17741635360323564569, "".as_bytes())
            .unwrap();
        builder
            .put("T".as_bytes(), 3442885773277545517, "".as_bytes())
            .unwrap();
        builder
            .put("U".as_bytes(), 16798869817908785490, "".as_bytes())
            .unwrap();
        builder.del("U".as_bytes(), 8329339752768468916).unwrap();
        builder
            .put("V".as_bytes(), 9966687898902172033, "".as_bytes())
            .unwrap();
        builder
            .put("W".as_bytes(), 13095774311180215755, "".as_bytes())
            .unwrap();
        builder
            .put("W".as_bytes(), 9347164485663886373, "".as_bytes())
            .unwrap();
        builder
            .put("X".as_bytes(), 14105912430424664753, "".as_bytes())
            .unwrap();
        builder
            .put("X".as_bytes(), 6418138334934602254, "".as_bytes())
            .unwrap();
        builder
            .put("X".as_bytes(), 55139404659432737, "".as_bytes())
            .unwrap();
        builder
            .put("Y".as_bytes(), 2104644631976488051, "".as_bytes())
            .unwrap();
        builder
            .put("Z".as_bytes(), 16236856772926750404, "".as_bytes())
            .unwrap();
        builder
            .put("Z".as_bytes(), 5615871050668577040, "".as_bytes())
            .unwrap();
        builder
            .put("a".as_bytes(), 3071821918069870007, "".as_bytes())
            .unwrap();
        builder
            .put("c".as_bytes(), 15097321419089962068, "".as_bytes())
            .unwrap();
        builder
            .put("c".as_bytes(), 8516680308564098410, "".as_bytes())
            .unwrap();
        builder
            .put("c".as_bytes(), 1136922606904185019, "".as_bytes())
            .unwrap();
        builder
            .put("d".as_bytes(), 11470523903049678620, "".as_bytes())
            .unwrap();
        builder
            .put("d".as_bytes(), 7780339209940962240, "".as_bytes())
            .unwrap();
        builder
            .put("e".as_bytes(), 11794849320489348897, "".as_bytes())
            .unwrap();
        builder
            .put("f".as_bytes(), 14643758144615450198, "".as_bytes())
            .unwrap();
        builder
            .put("g".as_bytes(), 10374159306796994843, "".as_bytes())
            .unwrap();
        builder
            .put("h".as_bytes(), 15699718780789327398, "".as_bytes())
            .unwrap();
        builder
            .put("k".as_bytes(), 4326521581274956632, "".as_bytes())
            .unwrap();
        builder
            .put("k".as_bytes(), 4092481979873166344, "".as_bytes())
            .unwrap();
        builder
            .put("l".as_bytes(), 16731700614287774313, "".as_bytes())
            .unwrap();
        builder
            .put("l".as_bytes(), 589255275485757846, "".as_bytes())
            .unwrap();
        builder
            .put("m".as_bytes(), 12311958346976601852, "".as_bytes())
            .unwrap();
        builder
            .put("m".as_bytes(), 4965766951128923512, "".as_bytes())
            .unwrap();
        builder
            .put("m".as_bytes(), 3693140343459290526, "".as_bytes())
            .unwrap();
        builder
            .put("m".as_bytes(), 735770394729692338, "".as_bytes())
            .unwrap();
        builder
            .put("n".as_bytes(), 12504712481410458650, "".as_bytes())
            .unwrap();
        builder
            .put("n".as_bytes(), 7535384965626452878, "".as_bytes())
            .unwrap();
        builder
            .put("p".as_bytes(), 11164631123798495192, "".as_bytes())
            .unwrap();
        builder
            .put("p".as_bytes(), 7904065694230536285, "".as_bytes())
            .unwrap();
        builder
            .put("p".as_bytes(), 2533648604198286980, "".as_bytes())
            .unwrap();
        builder
            .put("q".as_bytes(), 16221674258603117598, "".as_bytes())
            .unwrap();
        builder
            .put("q".as_bytes(), 15702955376497465948, "".as_bytes())
            .unwrap();
        builder
            .put("q".as_bytes(), 11880355228727610904, "".as_bytes())
            .unwrap();
        builder
            .put("q".as_bytes(), 3128143053549102168, "".as_bytes())
            .unwrap();
        builder
            .put("r".as_bytes(), 16352360294892915532, "".as_bytes())
            .unwrap();
        builder
            .put("r".as_bytes(), 5031220163138947161, "".as_bytes())
            .unwrap();
        builder
            .put("s".as_bytes(), 4251152130762342499, "".as_bytes())
            .unwrap();
        builder
            .put("s".as_bytes(), 383014263170880432, "".as_bytes())
            .unwrap();
        builder
            .put("t".as_bytes(), 15277352805187180008, "".as_bytes())
            .unwrap();
        builder
            .put("t".as_bytes(), 9106274701266412083, "".as_bytes())
            .unwrap();
        builder
            .put("t".as_bytes(), 7790837488841419319, "".as_bytes())
            .unwrap();
        builder
            .put("u".as_bytes(), 15023686233576793040, "".as_bytes())
            .unwrap();
        builder
            .put("u".as_bytes(), 13698086237460213740, "".as_bytes())
            .unwrap();
        builder
            .put("u".as_bytes(), 13011900067377589610, "".as_bytes())
            .unwrap();
        builder
            .put("u".as_bytes(), 12118947660501920842, "".as_bytes())
            .unwrap();
        builder
            .put("u".as_bytes(), 5277242483551738373, "".as_bytes())
            .unwrap();
        builder
            .put("v".as_bytes(), 4652147366029290205, "".as_bytes())
            .unwrap();
        builder
            .put("v".as_bytes(), 2133827469768204743, "".as_bytes())
            .unwrap();
        builder
            .put("x".as_bytes(), 733450490007248290, "".as_bytes())
            .unwrap();
        builder
            .put("y".as_bytes(), 13099064855710329456, "".as_bytes())
            .unwrap();
        builder
            .put("y".as_bytes(), 10455969331245208597, "".as_bytes())
            .unwrap();
        builder
            .put("y".as_bytes(), 10097328861729949124, "".as_bytes())
            .unwrap();
        builder
            .put("y".as_bytes(), 6129378363940112657, "".as_bytes())
            .unwrap();
        let block = builder.seal().unwrap();
        // Top of loop seeks to: Key { key: "6" }
        let mut cursor = block.cursor();
        cursor.seek("6".as_bytes()).unwrap();
        cursor.next().unwrap();
        cursor.next().unwrap();
        cursor.next().unwrap();
        let got = cursor.value().unwrap();
        let exp = KeyValueRef {
            key: "8".as_bytes(),
            timestamp: 13191224295862555992,
            value: Some("".as_bytes()),
        };
        assert_eq!(exp, got);
    }

    #[test]
    fn guacamole_5() {
        // --num-keys 10
        // --key-bytes 1
        // --value-bytes 0
        // --num-seeks 10
        // --seek-distance 1
        // --prev-probability 0.1
        let builder_opts = BlockBuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = BlockBuilder::new(builder_opts);
        builder
            .put("4".as_bytes(), 5220327133503220768, "".as_bytes())
            .unwrap();
        builder
            .put("A".as_bytes(), 2365635627947495809, "".as_bytes())
            .unwrap();
        builder
            .put("E".as_bytes(), 17563921251225492277, "".as_bytes())
            .unwrap();
        builder
            .put("I".as_bytes(), 3844377046565620216, "".as_bytes())
            .unwrap();
        builder
            .put("J".as_bytes(), 14848435744026832213, "".as_bytes())
            .unwrap();
        builder.del("U".as_bytes(), 8329339752768468916).unwrap();
        builder
            .put("g".as_bytes(), 10374159306796994843, "".as_bytes())
            .unwrap();
        builder
            .put("k".as_bytes(), 4092481979873166344, "".as_bytes())
            .unwrap();
        builder
            .put("t".as_bytes(), 7790837488841419319, "".as_bytes())
            .unwrap();
        builder
            .put("v".as_bytes(), 2133827469768204743, "".as_bytes())
            .unwrap();
        let block = builder.seal().unwrap();
        // Top of loop seeks to: "d"@4793296426793138773
        let mut cursor = block.cursor();
        cursor.seek("d".as_bytes()).unwrap();
        let _got = cursor.next().unwrap();
        // Top of loop seeks to: "I"@13021764449837349261
        let mut cursor = block.cursor();
        cursor.seek("I".as_bytes()).unwrap();
        cursor.prev().unwrap();
        let got = cursor.value().unwrap();
        let exp = KeyValueRef {
            key: "E".as_bytes(),
            timestamp: 17563921251225492277,
            value: Some("".as_bytes()),
        };
        assert_eq!(exp, got);
    }
}
