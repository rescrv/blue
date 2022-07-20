use std::cmp;
use std::cmp::Ordering;

use prototk::{length_free, stack_pack, Packable, Unpacker, v64};

use super::{compare_bytes,KeyValuePair,Iterator};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    BlockTooSmall{ length: usize, required: usize },
    UnpackError{ error: prototk::Error, context: String },
    Corruption{ context: String },
}

////////////////////////////////////////// BuilderOptions //////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct BuilderOptions {
    pub bytes_restart_interval: u64,
    pub key_value_pairs_restart_interval: u64,
}

impl Default for BuilderOptions {
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

////////////////////////////////////////////// Builder /////////////////////////////////////////////

// Build a block.
pub struct Builder {
    options: BuilderOptions,
    buffer: Vec<u8>,
    last_key: Vec<u8>,
    // Restart metadata.
    restarts: Vec<u32>,
    bytes_since_restart: u64,
    key_value_pairs_since_restart: u64,
}

impl Builder {
    pub fn new(options: BuilderOptions) -> Self {
        Self::reuse(options, Vec::default())
    }

    pub fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
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

    pub fn del(&mut self, key: &[u8], timestamp: u64) {
        let (shared, key_frag) = self.compute_key_frag(key);
        let kvp = KeyValueDel {
            shared: shared as u64,
            key_frag,
            timestamp,
        };
        let be = BlockEntry::KeyValueDel(kvp);
        self.append(be)
    }

    pub fn finish(mut self) -> Vec<u8> {
        // Append each restart.
        let restarts = length_free(&self.restarts);
        let tag10: v64  = ((10 << 3) | 2).into();
        let tag11: v64  = ((11 << 3) | 5).into();
        let sz: v64 = restarts.pack_sz().into();
        let pa = stack_pack(tag10);
        let pa = pa.pack(sz);
        let pa = pa.pack(restarts);
        let pa = pa.pack(tag11);
        let pa = pa.pack(self.restarts.len() as u32);
        pa.append_to_vec(&mut self.buffer);
        self.buffer
    }

    pub fn reuse(options: BuilderOptions, mut buffer: Vec<u8>) -> Self {
        buffer.truncate(0);
        let restarts = vec![0];
        Builder {
            options,
            buffer,
            last_key: Vec::default(),
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
    fn append<'a>(&mut self, be: BlockEntry<'a>) {
        // Update the last key.
        self.last_key.truncate(be.shared());
        self.last_key.extend_from_slice(be.key_frag());

        // Append to the vector.
        let pa = stack_pack(be);
        assert!(self.buffer.len() + pa.pack_sz() <= u32::max_value() as usize);
        pa.append_to_vec(&mut self.buffer);

        // Update the estimates for when we should do a restart.
        self.bytes_since_restart += pa.pack_sz() as u64;
        self.key_value_pairs_since_restart += 1;
    }
}

/////////////////////////////////////////////// Block //////////////////////////////////////////////

#[derive(Debug, Default)]
pub struct Block<'a> {
    // The raw bytes built by a builder.
    bytes: &'a [u8],

    // The restart intervals.  restarts_boundary points to the first restart point.
    restarts_boundary: usize,
    restarts_idx: usize,
    num_restarts: usize,
}

impl<'a> Block<'a> {
    pub fn new<'b: 'a>(bytes: &'b [u8]) -> Result<Self, Error> {
        // Load num_restarts.
        if bytes.len() < 4 {
            // This is impossible.  A block must end in a u32 that indicates how many restarts
            // there are.
            return Err(Error::BlockTooSmall { length: bytes.len(), required: 4 })
        }
        let mut up = Unpacker::new(&bytes[bytes.len() - 4..]);
        let num_restarts: u32 = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not read last four bytes of block".to_string() })?;
        let num_restarts: usize = num_restarts as usize;
        // Footer size.
        // |tag 10|v64 of num bytes|packed num_restarts u32s|tag 11|fixed32 capstone|
        let capstone: usize = 1/*tag 11*/ + 4/*fixed32 capstone*/;
        let footer_body: usize = num_restarts as usize * 4;
        let footer_head: usize = 1/*tag 10*/ + v64::from(footer_body).pack_sz();
        let restarts_idx = bytes.len() - capstone - footer_body;
        let restarts_boundary = restarts_idx - footer_head;
        // Block.
        let block = Block {
            bytes,
            restarts_boundary,
            restarts_idx,
            num_restarts,
        };
        Ok(block)
    }

    fn restart_point(&self, restart_idx: usize) -> usize {
        assert!(restart_idx < self.num_restarts as usize);
        let mut restart: [u8; 4] = <[u8; 4]>::default();
        for i in 0..4 {
            restart[i] = self.bytes[self.restarts_idx + restart_idx * 4 + i];
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
                },
                Ordering::Equal => {
                    // The offset exactly equals this restart point.  We're lucky that we can go
                    // home early.
                    left = mid;
                    right = mid;
                },
                Ordering::Greater => {
                    // The offset > value.  The best we can do is to move the left to the mid
                    // because it could still equal left.
                    left = mid;
                }
            }
        }

        self.restart_point(left)
    }
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

#[derive(Clone, Debug)]
pub enum Cursor<'a> {
    Head { block: &'a Block<'a> },
    Tail { block: &'a Block<'a> },
    Positioned {
        block: &'a Block<'a>,
        restart_idx: usize,
        offset: usize,
        next_offset: usize,
        key: Vec<u8>,
        timestamp: u64,
        value: Option<&'a [u8]>,
    }
}

impl<'a> Cursor<'a> {
    pub fn new(block: &'a Block<'a>) -> Self {
        Cursor::Head {
            block,
        }
    }

    fn block(&self) -> &'a Block<'a> {
        match self {
            Cursor::Head { block } => block,
            Cursor::Tail { block } => block,
            Cursor::Positioned { block, restart_idx: _, offset: _, next_offset: _, key: _, timestamp: _, value: _ } => block,
        }
    }

    fn next_offset(&self) -> usize {
        match self {
            Cursor::Head { block: _ } => 0,
            Cursor::Tail { block } => block.restarts_boundary,
            Cursor::Positioned { block: _, restart_idx: _, offset: _, next_offset, key: _, timestamp: _, value: _ } => *next_offset,
        }
    }

    fn restart_idx(&self) -> usize {
        match self {
            Cursor::Head { block: _ } => 0,
            Cursor::Tail { block } => block.restarts_boundary,
            Cursor::Positioned { block: _, restart_idx, offset: _, next_offset: _, key: _, timestamp: _, value: _ } => *restart_idx,
        }
    }

    fn seek_block(&mut self, restart_idx: usize) -> Result<KeyValuePair, Error> {
        if restart_idx >= self.block().num_restarts {
            return Err(Error::Corruption { context: format!("restart_idx={} exceeds num_restarts={}", restart_idx, self.block().num_restarts) });
        }
        let offset = self.block().restart_point(restart_idx) as usize;
        if offset >= self.block().restarts_boundary {
            return Err(Error::Corruption { context: format!("offset={} exceeds restarts_boundary={}", offset, self.block().restarts_boundary) });
        }
        let key = match self {
            Cursor::Head { block: _ } => Vec::new(),
            Cursor::Tail { block: _ } => Vec::new(),
            Cursor::Positioned { block: _, restart_idx: _, offset: _, next_offset: _, ref mut key, timestamp: _, value: _ } => {
                let mut ret = Vec::new();
                key.truncate(0);
                std::mem::swap(&mut ret, key);
                ret
            },
        };
        *self = Cursor::load_key_value(self.block(), offset, key)?;
        Ok(self.key_value_pair().unwrap())
    }

    fn load_key_value(block: &'a Block, offset: usize, mut key: Vec<u8>) -> Result<Cursor<'a>, Error> {
        // Check for overrun.
        if offset >= block.restarts_boundary {
            return Ok(Cursor::Tail { block });
        }
        // Parse `key_value`.
        let mut up = Unpacker::new(&block.bytes[offset..block.restarts_boundary]);
        let be: BlockEntry = up.unpack()
            .map_err(|e| Error::UnpackError{
                error: e,
                context: format!("could not unpack key-value pair at offset={}", offset),
            })?;
        let next_offset = block.restarts_boundary - up.remain().len();
        let restart_idx = block.restart_for_offset(offset);
        // Assemble the returnable cursor.
        key.truncate(be.shared());
        key.extend_from_slice(be.key_frag());
        let cursor = Cursor::Positioned {
            block: block,
            restart_idx,
            offset,
            next_offset,
            key,
            timestamp: be.timestamp(),
            value: be.value(),
        };
        Ok(cursor)
    }

    fn key_value_pair(&self) -> Option<KeyValuePair> {
        match self {
            Cursor::Head { block: _ } => { None },
            Cursor::Tail { block: _ } => { None },
            Cursor::Positioned { block: _, restart_idx: _, offset: _, next_offset: _, ref key, timestamp, value } => {
                let kvp = KeyValuePair {
                    key: &key,
                    timestamp: *timestamp,
                    value: *value,
                };
                Some(kvp)
            }
        }
    }
}

impl<'a> Iterator for Cursor<'a> {
    fn seek_to_first(&mut self) -> Result<(), super::Error> {
        *self = Cursor::Head {
            block: self.block(),
        };
        self.next()?;
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), super::Error> {
        *self = Cursor::Tail {
            block: self.block(),
        };
        self.prev()?;
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), super::Error> {
        if self.block().num_restarts == 0 {
            return Err(Error::Corruption { context: "a block with 0 restarts".to_string() }.into());
        }
        let mut left: usize = 0usize;
        let mut right: usize = self.block().num_restarts - 1;

        if right >= u32::max_value() as usize {
            return Err(Error::Corruption { context: "a block with too many restarts".to_string() }.into());
        }

        // Binary search to the correct block.
        while left < right {
            let mid = (left + right + 1) / 2;
            let kvp = self.seek_block(mid)?;
            match compare_bytes(key, &kvp.key) {
                Ordering::Less => {
                    // left     mid     right
                    // |--------|-------|
                    //       |
                    right = mid - 1;
                },
                Ordering::Equal => {
                    // left     mid     right
                    // |--------|-------|
                    //          |
                    // When the keys are equal, move left.  We don't move to mid - 1 in case the
                    // first of the key is in mid.
                    //
                    // NOTE(rescrv):  It's critical we don't bail early on equal.  If the key is
                    // equal we'll keep moving the right barrier until we hit the first key.  Left
                    // will never move to past the first key.
                    right = mid;
                },
                Ordering::Greater => {
                    // left     mid     right
                    // |--------|-------|
                    //           |
                    // NOTE(rescrv):  We move to mid because the +1 in the mid computation ensures
                    // we'll never have mid == left so long as left < right.  If we move to mid+1
                    // we will potentially run off the end of the original value of right.
                    left = mid;
                },
            };
        }

        let mut kvp = Some(self.seek_block(left)?);

        // Check for the case where all keys are bigger.
        if compare_bytes(key, kvp.as_ref().unwrap().key).is_lt() {
            *self = Cursor::Head {
                block: self.block(),
            };
            return Ok(());
        }

        // Scan until we find the key.
        while let Some(v) = kvp {
            if compare_bytes(key, v.key).is_gt() {
                kvp = self.next()?;
            } else {
                break;
            }
        }

        // Reposition the cursor so that a call to next or prev will bias it appropriately.
        match self {
            Cursor::Head { .. } => {},
            Cursor::Tail { .. } => {},
            Cursor::Positioned { block: _, restart_idx: _, offset, next_offset, key: _, timestamp: _, value: _ } => {
                *next_offset = *offset;
            },
        };

        Ok(())
    }

    fn prev(&mut self) -> Result<Option<KeyValuePair>, super::Error> {
        // The target offset where we won't proceed past.
        let target_offset = match self {
            Cursor::Head { block: _ } => { return Ok(None); },
            Cursor::Tail { block } => { block.restarts_boundary },
            Cursor::Positioned { block: _, restart_idx: _, offset, next_offset: _, key: _, timestamp: _, value: _ } => { *offset },
        };
        // Check for left-underrun.
        if target_offset <= 0 {
            *self = Cursor::Head {
                block: self.block(),
            };
            return Ok(None)
        }
        // If we happen to be at the boundary of a restart, step.
        let current_restart_idx = self.restart_idx();
        let restart_idx = if target_offset <= self.block().restart_point(current_restart_idx) as usize {
            if current_restart_idx == 0 {
                return Err(Error::Corruption {
                    context: format!("target_offset={} <= restarts[{}]={} on zero'th restart",
                                     target_offset, current_restart_idx, self.block().restart_point(current_restart_idx)),
                }.into());
            }
            current_restart_idx - 1
        } else {
            current_restart_idx
        };
        // If the restart index is out of bounds.
        if restart_idx >= self.block().restarts_boundary {
            return Err(Error::Corruption {
                context: format!("restart_idx={} exceeds restarts_boundary={}",
                                 restart_idx, self.block().restarts_boundary),
            }.into());
        }
        // Scan forward from the block index.
        self.seek_block(restart_idx)?;
        while self.next_offset() < target_offset {
            self.next()?;
        }
        self.same()
    }

    fn next(&mut self) -> Result<Option<KeyValuePair>, super::Error> {
        if let Cursor::Tail { block: _ } = self {
            return Ok(None);
        }
        let offset = self.next_offset();
        if offset >= self.block().restarts_boundary {
            *self = Cursor::Tail {
                block: self.block()
            };
            return Ok(None);
        }
        if self.restart_idx() + 1 < self.block().num_restarts && self.block().restart_point(self.restart_idx() + 1) as usize <= offset {
            // We're jumping to the next block, so just seek_block to force a refresh of the key,
            // along with safety checks on key_frag.
            let value = self.seek_block(self.restart_idx() + 1)?;
            return Ok(Some(value));
        };
        let key = match self {
            Cursor::Head { block: _ } => Vec::new(),
            Cursor::Tail { block: _ } => Vec::new(),
            Cursor::Positioned { block: _, restart_idx: _, offset: _, next_offset: _, ref mut key, timestamp: _, value: _ } => {
                let mut ret = Vec::new();
                std::mem::swap(&mut ret, key);
                ret
            },
        };
        *self = Cursor::load_key_value(self.block(), offset, key)?;
        self.same()
    }

    fn same(&mut self) -> Result<Option<KeyValuePair>, super::Error> {
        Ok(self.key_value_pair())
    }
}

impl<'a> PartialEq for Cursor<'a> {
    fn eq(&self, rhs: &Cursor<'a>) -> bool {
        match (self, rhs) {
            (&Cursor::Head { block: _ }, &Cursor::Head { block: _ }) => { true },
            (&Cursor::Tail { block: _ }, &Cursor::Tail { block: _ }) => { true },
            (&Cursor::Positioned { block: _, restart_idx: ri1, offset: o1, next_offset: no1, key: ref k1, timestamp: t1, value: v1 },
             &Cursor::Positioned { block: _, restart_idx: ri2, offset: o2, next_offset: no2, key: ref k2, timestamp: t2, value: v2 }) => {
                ri1 == ri2 && o1 == o2 && no1 == no2 && k1 == k2 && t1 == t2 && v1 == v2
            },
            _ => { false }
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_empty_block() {
        let block = Builder::new(BuilderOptions::default());
        let finisher = block.finish();
        let got = finisher.as_slice();
        let exp = &[82, 4, 0, 0, 0, 0, 93, 1, 0, 0, 0];
        assert_eq!(exp, got);
        assert_eq!(11, got.len());
    }

    #[test]
    fn build_single_item_block() {
        let mut block = Builder::new(BuilderOptions::default());
        block.put("key".as_bytes(), 0xc0ffee, "value".as_bytes());
        let finisher = block.finish();
        let got = finisher.as_slice();
        let exp = &[
            66/*8*/, 19/*sz*/,
            8/*1*/, 0/*zero*/,
            18/*2*/, 3/*sz*/, 107, 101, 121,
            24/*3*/, /*varint(0xc0ffee):*/238, 255, 131, 6,
            34/*4*/, 5/*sz*/, 118, 97, 108, 117, 101,
            // restarts
            82/*10*/, 4/*sz*/,
            0, 0, 0, 0,
            // num_restarts
            93/*11*/,
            1, 0, 0, 0,
        ];
        assert_eq!(exp, got);
    }

    #[test]
    fn build_prefix_compression() {
        let mut block = Builder::new(BuilderOptions::default());
        block.put("key1".as_bytes(), 0xc0ffee, "value1".as_bytes());
        block.put("key2".as_bytes(), 0xc0ffee, "value2".as_bytes());
        let finisher = block.finish();
        let got = finisher.as_slice();
        let exp = &[
            // first record
            66/*8*/, 21/*sz*/,
            8/*1*/, 0,
            18/*2*/, 4/*sz*/, 107, 101, 121, 49,
            24/*3*/, /*varint(0xc0ffee)*/238, 255, 131, 6,
            34/*4*/, 6/*sz*/, 118, 97, 108, 117, 101, 49,

            // second record
            66/*8*/, 18/*sz*/,
            8/*1*/, 3,
            18/*2*/, 1/*sz*/, 50,
            24/*3*/, /*varint(0xc0ffee)*/238, 255, 131, 6,
            34/*4*/, 6/*sz*/, 118, 97, 108, 117, 101, 50,

            // restarts
            82/*10*/, 4/*sz*/,
            0, 0, 0, 0,
            93/*11*/,
            1, 0, 0, 0,
        ];
        assert_eq!(exp, got);
    }

    #[test]
    fn load_restart_points() {
        let block_bytes = &[
            // first record
            66/*8*/, 21/*sz*/,
            8/*1*/, 0,
            18/*2*/, 4/*sz*/, 107, 101, 121, 49,
            24/*3*/, /*varint(0xc0ffee)*/238, 255, 131, 6,
            34/*4*/, 6/*sz*/, 118, 97, 108, 117, 101, 49,

            // second record
            66/*8*/, 21/*sz*/,
            8/*1*/, 0,
            18/*2*/, 4/*sz*/, 107, 101, 121, 50,
            24/*3*/, /*varint(0xc0ffee)*/238, 255, 131, 6,
            34/*4*/, 6/*sz*/, 118, 97, 108, 117, 101, 50,

            // restarts
            82/*10*/, 8/*sz*/,
            0, 0, 0, 0,
            22, 0, 0, 0,
            93/*11*/,
            2, 0, 0, 0,
        ];
        let block = Block::new(block_bytes).unwrap();
        assert_eq!(2, block.num_restarts);
        assert_eq!(0, block.restart_point(0));
        assert_eq!(22, block.restart_point(1));
    }

    #[test]
    fn corruption_bug_gone() {
        let key = &[107, 65, 118, 119, 82, 109, 53, 69];
        let timestamp = 4092481979873166344;
        let value = &[120, 100, 81, 80, 75, 79, 121, 90];
        let mut block = Builder::new(BuilderOptions::default());
        block.put(key, timestamp, value);
        let finisher = block.finish();
        let exp = &[
            // record
            66/*8*/, 32/*sz*/,
            8/*1*/, 0,
            18/*2*/, 8/*sz*/, 107, 65, 118, 119, 82, 109, 53, 69,
            24/*3*/, /*varint*/136, 136, 156, 160, 216, 213, 218, 229, 56,
            34/*4*/, 8/*sz*/, 120, 100, 81, 80, 75, 79, 121, 90,

            // restarts
            82/*10*/, 4/*sz*/,
            0, 0, 0, 0,
            93/*11*/,
            1, 0, 0, 0,
        ];
        let got = finisher.as_slice();
        assert_eq!(exp, got);

        let block = Block::new(&got).unwrap();
        let mut cursor = Cursor::new(&block);
        cursor.seek(&[106, 113, 67, 73, 122, 73, 98, 85]).unwrap();
    }

    #[test]
    fn seek_bug_gone() {
        let key = "kAvwRm5E";
        let timestamp = 4092481979873166344;
        let value = "xdQPKOyZwQUykR8i";

        let mut block = Builder::new(BuilderOptions::default());
        block.put(key.as_bytes(), timestamp, value.as_bytes());
        let finisher = block.finish();

        let block = Block::new(finisher.as_slice()).unwrap();
        let mut cursor = Cursor::new(&block);
        let target = "jqCIzIbU";
        cursor.seek(target.as_bytes()).unwrap();
        let kvp = cursor.next().unwrap().unwrap();
        assert_eq!(key.as_bytes(), kvp.key);
        assert_eq!(timestamp, kvp.timestamp);
        assert_eq!(Some(value.as_bytes()), kvp.value);
    }

    #[test]
    fn cursor_equals() {
        let mut builder = Builder::new(BuilderOptions::default());
        builder.put("E".as_bytes(), 17563921251225492277, "".as_bytes());
        let finished = builder.finish();
        let block = &Block::new(finished.as_slice()).unwrap();

        let lhs = Cursor::Head { block };
        let rhs = Cursor::Head { block };
        assert_eq!(lhs, rhs);

        let lhs = Cursor::Tail { block };
        let rhs = Cursor::Tail { block };
        assert_eq!(lhs, rhs);

        let lhs = Cursor::Positioned {
            block,
            restart_idx: 0,
            offset: 0,
            next_offset: 19,
            key: "E".as_bytes().to_vec(),
            timestamp: 17563921251225492277,
            value: Some("".as_bytes()),
        };
        let rhs = Cursor::Positioned {
            block,
            restart_idx: 0,
            offset: 0,
            next_offset: 19,
            key: "E".as_bytes().to_vec(),
            timestamp: 17563921251225492277,
            value: Some("".as_bytes()),
        };
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn load_key_value() {
		let bytes = &[
            // record
            66/*8*/, 18/*sz*/,
            8/*1*/, 0,
            18/*2*/, 1/*sz*/, 69,
            24/*3*/, /*varint*/181, 182, 235, 145, 160, 170, 229, 223, 243, 1,
            34/*4*/, 0/*sz*/,

			// record
			66/*8*/, 17/*sz*/,
			8/*1*/, 0,
			18/*2*/, 1/*sz*/, 107,
			24/*3*/, /*varint*/136, 136, 156, 160, 216, 213, 218, 229, 56,
			34/*4*/, 0/*sz*/,

            // restarts
            82/*10*/, 4/*sz*/,
            0, 0, 0, 0,
            93/*11*/,
            1, 0, 0, 0,
		];
        let block = Block::new(bytes).unwrap();

        let exp = Cursor::Positioned {
            block: &block,
            restart_idx: 0,
            offset: 0,
            next_offset: 20,
            key: "E".as_bytes().to_vec(),
            timestamp: 17563921251225492277,
            value: Some("".as_bytes()),
        };
        let got = Cursor::load_key_value(&block, 0, Vec::new()).unwrap();
        assert_eq!(exp, got);

        let exp = Cursor::Positioned {
            block: &block,
            restart_idx: 0,
            offset: 20,
            next_offset: 39,
            key: "k".as_bytes().to_vec(),
            timestamp: 4092481979873166344,
            value: Some("".as_bytes()),
        };
        let got = Cursor::load_key_value(&block, 20, Vec::new()).unwrap();
        assert_eq!(exp, got);

        let exp = Cursor::Tail { block: &block };
        let got = Cursor::load_key_value(&block, 39, Vec::new()).unwrap();
        assert_eq!(exp, got);
    }

    #[test]
    fn human_guacamole_1() {
        // --num-keys 2
        // --key-bytes 1
        // --value-bytes 0
        // --num-seeks 1000
        // --seek-distance 10
        let builder_opts = BuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = Builder::new(builder_opts);
        builder.put("E".as_bytes(), 17563921251225492277, "".as_bytes());
        builder.put("k".as_bytes(), 4092481979873166344, "".as_bytes());
        let finished = builder.finish();

        let block = Block::new(finished.as_slice()).unwrap();
		let exp = [
            // record
            66/*8*/, 18/*sz*/,
            8/*1*/, 0,
            18/*2*/, 1/*sz*/, 69,
            24/*3*/, /*varint*/181, 182, 235, 145, 160, 170, 229, 223, 243, 1,
            34/*4*/, 0/*sz*/,

			// record
			66/*8*/, 17/*sz*/,
			8/*1*/, 0,
			18/*2*/, 1/*sz*/, 107,
			24/*3*/, /*varint*/136, 136, 156, 160, 216, 213, 218, 229, 56,
			34/*4*/, 0/*sz*/,

            // restarts
            82/*10*/, 4/*sz*/,
            0, 0, 0, 0,
            93/*11*/,
            1, 0, 0, 0,
		];
		assert_eq!(exp, block.bytes);

        let mut cursor = Cursor::new(&block);
        match cursor {
            Cursor::Head { .. } => {},
            _ => { panic!("cursor should always init to head: {:?}", cursor) },
        };
        cursor.seek("t".as_bytes()).unwrap();
        match cursor {
            Cursor::Tail { .. } => {},
            _ => { panic!("cursor should seek to the end: {:?}", cursor) },
        };
        let got = cursor.next().unwrap();
        assert_eq!(None, got);
    }

	#[test]
    fn human_guacamole_2() {
        // --num-keys 10
        // --key-bytes 1
        // --value-bytes 64
        // --num-seeks 1
        // --seek-distance 4
        let builder_opts = BuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = Builder::new(builder_opts);
        builder.put("4".as_bytes(), 5220327133503220768, "TFJaKOq4itZUjZ6zLYRQAtaYQJ2KOABpaX5Jxr07mN9NgTFUN70JdcuwGubnsBSV".as_bytes());
        builder.put("A".as_bytes(), 2365635627947495809, "JMbW18opQPCC6OsP5XSbF5bs9LWzNwSjS2uQKhkDv7rATMznKwv6yA5jWq0Ya77j".as_bytes());
        builder.put("E".as_bytes(), 17563921251225492277, "ZVaW3VAlMCSMzUF7lOFVun1pObMORRWajFd0gvzfK1Qwtyp0L8GnEfN1TBoDgG6v".as_bytes());
        builder.put("I".as_bytes(), 3844377046565620216, "0lfqYezeQ1mM8HYtpTNLVB4XQi8KAb2ouxCTLHjMTzGxBFaHuVVY1Osd23MrzSA6".as_bytes());
        builder.put("J".as_bytes(), 14848435744026832213, "RH53KxwpLPbrUJat64bFvDMqLXVEXfxwL1LAfVBVzcbsEd5QaIzUyPfhuIOvcUiw".as_bytes());
        builder.del("U".as_bytes(), 8329339752768468916);
        builder.put("g".as_bytes(), 10374159306796994843, "SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes());
        builder.put("k".as_bytes(), 4092481979873166344, "xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes());
        builder.put("t".as_bytes(), 7790837488841419319, "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes());
        builder.put("v".as_bytes(), 2133827469768204743, "5NV1fDTU6IBuTs5qP7mdDRrBlMCUlsVzXrk8dbMTjhrzdEaLtOSuC5sL3401yvrs".as_bytes());
        let finisher = builder.finish();
        let block = Block::new(finisher.as_slice()).unwrap();
        // Top of loop seeks to: Key { key: "d" }
        let mut cursor = Cursor::new(&block);
        cursor.seek("d".as_bytes()).unwrap();
        // Next to g
        let got = cursor.next().unwrap().unwrap();
        assert_eq!("g".as_bytes(), got.key);
        assert_eq!(10374159306796994843, got.timestamp);
        assert_eq!(Some("SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes()), got.value);
        assert_eq!(Cursor::Positioned {
            block: &block,
            restart_idx: 0,
            offset: 434,
            next_offset: 518,
            key: "g".as_bytes().to_vec(),
            timestamp: 10374159306796994843,
            value: Some("SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes()),
        }, cursor);
        // Next to k
        let got = cursor.next().unwrap().unwrap();
        assert_eq!("k".as_bytes(), got.key);
        assert_eq!(4092481979873166344, got.timestamp);
        assert_eq!(Some("xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes()), got.value);
        assert_eq!(Cursor::Positioned {
            block: &block,
            restart_idx: 518,
            offset: 518,
            next_offset: 601,
            key: "k".as_bytes().to_vec(),
            timestamp: 4092481979873166344,
            value: Some("xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes()),
        }, cursor);
        // Next to t
        let got = cursor.next().unwrap().unwrap();
        let exp = KeyValuePair {
            key: "t".as_bytes(),
            timestamp: 7790837488841419319,
            value: Some("mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes()),
        };
        assert_eq!(exp, got);
        assert_eq!("t".as_bytes(), got.key);
        assert_eq!(7790837488841419319, got.timestamp);
        assert_eq!(Some("mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes()), got.value);
        assert_eq!(Cursor::Positioned {
            block: &block,
            restart_idx: 518,
            offset: 601,
            next_offset: 684,
            key: "t".as_bytes().to_vec(),
            timestamp: 7790837488841419319,
            value: Some("mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes()),
        }, cursor);
    }

	#[test]
    fn guacamole_2() {
        // --num-keys 10
        // --key-bytes 1
        // --value-bytes 64
        // --num-seeks 1
        // --seek-distance 4
        let builder_opts = BuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = Builder::new(builder_opts);
        builder.put("4".as_bytes(), 5220327133503220768, "TFJaKOq4itZUjZ6zLYRQAtaYQJ2KOABpaX5Jxr07mN9NgTFUN70JdcuwGubnsBSV".as_bytes());
        builder.put("A".as_bytes(), 2365635627947495809, "JMbW18opQPCC6OsP5XSbF5bs9LWzNwSjS2uQKhkDv7rATMznKwv6yA5jWq0Ya77j".as_bytes());
        builder.put("E".as_bytes(), 17563921251225492277, "ZVaW3VAlMCSMzUF7lOFVun1pObMORRWajFd0gvzfK1Qwtyp0L8GnEfN1TBoDgG6v".as_bytes());
        builder.put("I".as_bytes(), 3844377046565620216, "0lfqYezeQ1mM8HYtpTNLVB4XQi8KAb2ouxCTLHjMTzGxBFaHuVVY1Osd23MrzSA6".as_bytes());
        builder.put("J".as_bytes(), 14848435744026832213, "RH53KxwpLPbrUJat64bFvDMqLXVEXfxwL1LAfVBVzcbsEd5QaIzUyPfhuIOvcUiw".as_bytes());
        builder.del("U".as_bytes(), 8329339752768468916);
        builder.put("g".as_bytes(), 10374159306796994843, "SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes());
        builder.put("k".as_bytes(), 4092481979873166344, "xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes());
        builder.put("t".as_bytes(), 7790837488841419319, "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes());
        builder.put("v".as_bytes(), 2133827469768204743, "5NV1fDTU6IBuTs5qP7mdDRrBlMCUlsVzXrk8dbMTjhrzdEaLtOSuC5sL3401yvrs".as_bytes());
        let finisher = builder.finish();
        let block = Block::new(finisher.as_slice()).unwrap();
        // Top of loop seeks to: Key { key: "d" }
        let mut cursor = Cursor::new(&block);
        cursor.seek("d".as_bytes()).unwrap();
        let _got = cursor.next().unwrap();
        let _got = cursor.next().unwrap();
        let got = cursor.next().unwrap();
        let exp = KeyValuePair {
            key: "t".as_bytes(),
            timestamp: 7790837488841419319,
            value: Some("mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes()),
        };
        assert_eq!(Some(exp), got);
    }

	#[test]
    fn human_guacamole_3() {
        // --num-keys 10
        // --key-bytes 1
        // --value-bytes 64
        // --num-seeks 10
        // --seek-distance 1
        let builder_opts = BuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = Builder::new(builder_opts);
        builder.put("4".as_bytes(), 5220327133503220768, "TFJaKOq4itZUjZ6zLYRQAtaYQJ2KOABpaX5Jxr07mN9NgTFUN70JdcuwGubnsBSV".as_bytes());
        builder.put("A".as_bytes(), 2365635627947495809, "JMbW18opQPCC6OsP5XSbF5bs9LWzNwSjS2uQKhkDv7rATMznKwv6yA5jWq0Ya77j".as_bytes());
        builder.put("E".as_bytes(), 17563921251225492277, "ZVaW3VAlMCSMzUF7lOFVun1pObMORRWajFd0gvzfK1Qwtyp0L8GnEfN1TBoDgG6v".as_bytes());
        builder.put("I".as_bytes(), 3844377046565620216, "0lfqYezeQ1mM8HYtpTNLVB4XQi8KAb2ouxCTLHjMTzGxBFaHuVVY1Osd23MrzSA6".as_bytes());
        builder.put("J".as_bytes(), 14848435744026832213, "RH53KxwpLPbrUJat64bFvDMqLXVEXfxwL1LAfVBVzcbsEd5QaIzUyPfhuIOvcUiw".as_bytes());
        builder.del("U".as_bytes(), 8329339752768468916);
        builder.put("g".as_bytes(), 10374159306796994843, "SlJsi4yMZ6KanbWHPvrdPIFbMIl5jvGCETwcklFf2w8b0GsN4dyIdIsB1KlTPwgO".as_bytes());
        builder.put("k".as_bytes(), 4092481979873166344, "xdQPKOyZwQUykR8iVbMtYMhEaiW3jbrS5AKqteHkjnRs2Yfl4OOqtvVQKqojsB0a".as_bytes());
        builder.put("t".as_bytes(), 7790837488841419319, "mXdsaM4QhryUTwpDzkUhYqxfoQ9BWK1yjRZjQxF4ls6tV4r8K5G7Rpk1ZLNPcsFl".as_bytes());
        builder.put("v".as_bytes(), 2133827469768204743, "5NV1fDTU6IBuTs5qP7mdDRrBlMCUlsVzXrk8dbMTjhrzdEaLtOSuC5sL3401yvrs".as_bytes());
        let finisher = builder.finish();
        let block = Block::new(finisher.as_slice()).unwrap();
        // Top of loop seeks to: Key { key: "u" }
        let mut cursor = Cursor::new(&block);
        cursor.seek("u".as_bytes()).unwrap();
	}

    #[test]
    fn guacamole_4() {
        // --num-keys 100
        // --key-bytes 1
        // --value-bytes 0
        // --num-seeks 1
        // --seek-distance 4
        let builder_opts = BuilderOptions {
            bytes_restart_interval: 512,
            key_value_pairs_restart_interval: 16,
        };
        let mut builder = Builder::new(builder_opts);
        builder.put("0".as_bytes(), 9697512111035884403, "".as_bytes());
        builder.put("1".as_bytes(), 3798246989967619197, "".as_bytes());
        builder.put("2".as_bytes(), 10342091538431028726, "".as_bytes());
        builder.put("3".as_bytes(), 15157365073906098091, "".as_bytes());
        builder.put("3".as_bytes(), 9466660179799601223, "".as_bytes());
        builder.put("3".as_bytes(), 5028655377053437110, "".as_bytes());
        builder.put("4".as_bytes(), 16805872069322243742, "".as_bytes());
        builder.put("4".as_bytes(), 16112959034514062976, "".as_bytes());
        builder.put("4".as_bytes(), 7876299547345770848, "".as_bytes());
        builder.put("4".as_bytes(), 5220327133503220768, "".as_bytes());
        builder.put("7".as_bytes(), 14395010029865413065, "".as_bytes());
        builder.put("8".as_bytes(), 17618669414409465042, "".as_bytes());
        builder.put("8".as_bytes(), 13191224295862555992, "".as_bytes());
        builder.put("8".as_bytes(), 5084626311153408505, "".as_bytes());
        builder.put("9".as_bytes(), 12995477672441385068, "".as_bytes());
        builder.put("A".as_bytes(), 9605838007579610207, "".as_bytes());
        builder.put("A".as_bytes(), 2365635627947495809, "".as_bytes());
        builder.put("A".as_bytes(), 1952263260996816483, "".as_bytes());
        builder.put("B".as_bytes(), 10126582942351468573, "".as_bytes());
        builder.put("C".as_bytes(), 16217491379957293402, "".as_bytes());
        builder.put("C".as_bytes(), 1973107251517101738, "".as_bytes());
        builder.put("E".as_bytes(), 17563921251225492277, "".as_bytes());
        builder.put("F".as_bytes(), 7744344282933500472, "".as_bytes());
        builder.put("F".as_bytes(), 7572175103299679188, "".as_bytes());
        builder.put("G".as_bytes(), 3562951228830167005, "".as_bytes());
        builder.put("H".as_bytes(), 10415469497441400582, "".as_bytes());
        builder.put("I".as_bytes(), 3844377046565620216, "".as_bytes());
        builder.put("J".as_bytes(), 17476236525666259675, "".as_bytes());
        builder.put("J".as_bytes(), 14848435744026832213, "".as_bytes());
        builder.put("K".as_bytes(), 5137225721270789888, "".as_bytes());
        builder.put("K".as_bytes(), 4825960407565437069, "".as_bytes());
        builder.put("L".as_bytes(), 15335622082534854763, "".as_bytes());
        builder.put("L".as_bytes(), 7211574025721472487, "".as_bytes());
        builder.put("M".as_bytes(), 485375931245920424, "".as_bytes());
        builder.put("O".as_bytes(), 6226508136092163051, "".as_bytes());
        builder.put("P".as_bytes(), 11429503906557966656, "".as_bytes());
        builder.put("P".as_bytes(), 6890969690330950371, "".as_bytes());
        builder.put("P".as_bytes(), 1488139426474409410, "".as_bytes());
        builder.put("P".as_bytes(), 418483046145178590, "".as_bytes());
        builder.put("R".as_bytes(), 13695467658803848996, "".as_bytes());
        builder.put("R".as_bytes(), 9039056961022621355, "".as_bytes());
        builder.put("T".as_bytes(), 17741635360323564569, "".as_bytes());
        builder.put("T".as_bytes(), 3442885773277545517, "".as_bytes());
        builder.put("U".as_bytes(), 16798869817908785490, "".as_bytes());
        builder.del("U".as_bytes(), 8329339752768468916);
        builder.put("V".as_bytes(), 9966687898902172033, "".as_bytes());
        builder.put("W".as_bytes(), 13095774311180215755, "".as_bytes());
        builder.put("W".as_bytes(), 9347164485663886373, "".as_bytes());
        builder.put("X".as_bytes(), 14105912430424664753, "".as_bytes());
        builder.put("X".as_bytes(), 6418138334934602254, "".as_bytes());
        builder.put("X".as_bytes(), 55139404659432737, "".as_bytes());
        builder.put("Y".as_bytes(), 2104644631976488051, "".as_bytes());
        builder.put("Z".as_bytes(), 16236856772926750404, "".as_bytes());
        builder.put("Z".as_bytes(), 5615871050668577040, "".as_bytes());
        builder.put("a".as_bytes(), 3071821918069870007, "".as_bytes());
        builder.put("c".as_bytes(), 15097321419089962068, "".as_bytes());
        builder.put("c".as_bytes(), 8516680308564098410, "".as_bytes());
        builder.put("c".as_bytes(), 1136922606904185019, "".as_bytes());
        builder.put("d".as_bytes(), 11470523903049678620, "".as_bytes());
        builder.put("d".as_bytes(), 7780339209940962240, "".as_bytes());
        builder.put("e".as_bytes(), 11794849320489348897, "".as_bytes());
        builder.put("f".as_bytes(), 14643758144615450198, "".as_bytes());
        builder.put("g".as_bytes(), 10374159306796994843, "".as_bytes());
        builder.put("h".as_bytes(), 15699718780789327398, "".as_bytes());
        builder.put("k".as_bytes(), 4326521581274956632, "".as_bytes());
        builder.put("k".as_bytes(), 4092481979873166344, "".as_bytes());
        builder.put("l".as_bytes(), 16731700614287774313, "".as_bytes());
        builder.put("l".as_bytes(), 589255275485757846, "".as_bytes());
        builder.put("m".as_bytes(), 12311958346976601852, "".as_bytes());
        builder.put("m".as_bytes(), 4965766951128923512, "".as_bytes());
        builder.put("m".as_bytes(), 3693140343459290526, "".as_bytes());
        builder.put("m".as_bytes(), 735770394729692338, "".as_bytes());
        builder.put("n".as_bytes(), 12504712481410458650, "".as_bytes());
        builder.put("n".as_bytes(), 7535384965626452878, "".as_bytes());
        builder.put("p".as_bytes(), 11164631123798495192, "".as_bytes());
        builder.put("p".as_bytes(), 7904065694230536285, "".as_bytes());
        builder.put("p".as_bytes(), 2533648604198286980, "".as_bytes());
        builder.put("q".as_bytes(), 16221674258603117598, "".as_bytes());
        builder.put("q".as_bytes(), 15702955376497465948, "".as_bytes());
        builder.put("q".as_bytes(), 11880355228727610904, "".as_bytes());
        builder.put("q".as_bytes(), 3128143053549102168, "".as_bytes());
        builder.put("r".as_bytes(), 16352360294892915532, "".as_bytes());
        builder.put("r".as_bytes(), 5031220163138947161, "".as_bytes());
        builder.put("s".as_bytes(), 4251152130762342499, "".as_bytes());
        builder.put("s".as_bytes(), 383014263170880432, "".as_bytes());
        builder.put("t".as_bytes(), 15277352805187180008, "".as_bytes());
        builder.put("t".as_bytes(), 9106274701266412083, "".as_bytes());
        builder.put("t".as_bytes(), 7790837488841419319, "".as_bytes());
        builder.put("u".as_bytes(), 15023686233576793040, "".as_bytes());
        builder.put("u".as_bytes(), 13698086237460213740, "".as_bytes());
        builder.put("u".as_bytes(), 13011900067377589610, "".as_bytes());
        builder.put("u".as_bytes(), 12118947660501920842, "".as_bytes());
        builder.put("u".as_bytes(), 5277242483551738373, "".as_bytes());
        builder.put("v".as_bytes(), 4652147366029290205, "".as_bytes());
        builder.put("v".as_bytes(), 2133827469768204743, "".as_bytes());
        builder.put("x".as_bytes(), 733450490007248290, "".as_bytes());
        builder.put("y".as_bytes(), 13099064855710329456, "".as_bytes());
        builder.put("y".as_bytes(), 10455969331245208597, "".as_bytes());
        builder.put("y".as_bytes(), 10097328861729949124, "".as_bytes());
        builder.put("y".as_bytes(), 6129378363940112657, "".as_bytes());
        let finisher = builder.finish();
        let block = Block::new(finisher.as_slice()).unwrap();
        // Top of loop seeks to: Key { key: "6" }
        let mut cursor = Cursor::new(&block);
        cursor.seek("6".as_bytes()).unwrap();
        let got = cursor.next().unwrap();
        let got = cursor.next().unwrap();
        let got = cursor.next().unwrap();
        let exp = KeyValuePair {
            key: "8".as_bytes(),
            timestamp: 13191224295862555992,
            value: Some("".as_bytes()),
        };
        assert_eq!(Some(exp), got);
    }

    // TODO(rescrv): Test empty tables.
}
