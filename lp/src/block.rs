use std::cmp;
use std::cmp::Ordering;

use prototk::{length_free, stack_pack, Packable, Unpacker, v64};

use super::{KeyValuePair,Iterator};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    BlockTooSmall{ length: usize, required: usize },
    UnpackError{ error: prototk::Error, context: String },
    Corruption{ context: String },
}

////////////////////////////////////////// BuilderOptions //////////////////////////////////////////

#[derive(Clone)]
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

#[derive(Clone, Default, Message)]
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

#[derive(Clone, Default, Message)]
struct KeyValueDel<'a> {
    #[prototk(5, uint64)]
    shared: u64,
    #[prototk(6, bytes)]
    key_frag: &'a [u8],
    #[prototk(7, uint64)]
    timestamp: u64,
}

//////////////////////////////////////////// BlockEntry ////////////////////////////////////////////

#[derive(Clone, Message)]
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
            BlockEntry::KeyValueDel(x) => None,
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
    // TODO(rescrv):  Lazily load this.
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
        let sz = pa.pack_sz();
        let pa = pa.pack(sz as u32);
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
            // Assert that keys go in order.
            // Either we ran out the end of the shared space and the last key is shorter than the
            // current key, or we hit a division point where the keys diverged before their common
            // length
            assert!(
                (shared == max_shared && self.last_key.len() < key.len())
                    || self.last_key[shared] < key[shared]
            );
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

#[derive(Default)]
pub struct Block<'a> {
    // The raw bytes built by a builder.
    bytes: &'a [u8],

    // The restart intervals.  restarts_boundary points to the first restart point.
    restarts_boundary: usize,
    num_restarts: u32,
    restarts: Vec<u32>,
}

impl<'a> Block<'a> {
    pub fn new<'b: 'a>(bytes: &'b [u8]) -> Result<Self, Error> {
        // Load the restarts.
        if bytes.len() < 4 {
            // This is impossible.  A block must end in a u32 that indicates how many restarts
            // there are.
            return Err(Error::BlockTooSmall { length: bytes.len(), required: 4 })
        }
        let mut up = Unpacker::new(&bytes[bytes.len() - 4..]);
        // Footer size.
        let num_restarts: u32 = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not read last four bytes of block".to_string() })?;
        let restarts_sz = num_restarts as usize * 4;
        let footer_sz: usize = restarts_sz + 1/*capstone tag*/ + 4/*fixed32 capstone*/;
        if bytes.len() < footer_sz {
            return Err(Error::BlockTooSmall { length: bytes.len(), required: footer_sz })
        }
        let restarts_boundary = bytes.len() - footer_sz;
        let mut restarts = Vec::new();
        let mut up = Unpacker::new(&bytes[restarts_boundary..]);
        // TODO(rescrv):  It would be awesome to do something unsafe and treat this as an array
        // rather than something to load at startup.
        for _ in 0..num_restarts {
            let x: u32 = up.unpack()
                .map_err(|e| Error::UnpackError{ error: e, context: "could not read restart points".to_string() })?;
            restarts.push(x);
        }
        // TODO(rescrv):  Decide how to error if zero restarts.
        let block = Block {
            bytes,
            restarts_boundary,
            num_restarts,
            restarts,
        };
        Ok(block)
    }

    fn restart_point(&self, restart_idx: usize) -> usize {
        assert!(restart_idx < self.num_restarts as usize);
        let restarts_sz = self.num_restarts as usize * 4;
        let footer_sz: usize = restarts_sz + 1/*capstone tag*/ + 4/*fixed32 capstone*/;
        let mut restart: [u8; 4] = <[u8; 4]>::default();
        for i in 0..4 {
            restart[i] = self.bytes[self.restarts_boundary + restart_idx * 4 + i];
        }
        let restart = u32::from_le_bytes(restart);
        assert_eq!(self.restarts[restart_idx], restart);
        self.restarts[restart_idx] as usize
    }
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

#[derive(Clone)]
pub enum Cursor<'a> {
    Head { block: &'a Block<'a> },
    Tail { block: &'a Block<'a> },
    Positioned {
        block: &'a Block<'a>,
        restart_idx: usize,
        offset: usize, 
        next_offset: usize,
        key: Vec<u8>,
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
            Cursor::Positioned { block, restart_idx: _, offset: _, next_offset: _, key: _ } => block,
        }
    }

    fn next_offset(&self) -> usize {
        match self {
            Cursor::Head { block: _ } => 0,
            Cursor::Tail { block } => block.restarts_boundary,
            Cursor::Positioned { block: _, restart_idx: _, offset: _, next_offset, key: _ } => *next_offset,
        }
    }

    fn restart_idx(&self) -> usize {
        match self {
            Cursor::Head { block: _ } => 0,
            Cursor::Tail { block } => block.restarts_boundary,
            Cursor::Positioned { block: _, restart_idx, offset: _, next_offset: _, key: _ } => *restart_idx,
        }
    }

    fn key(&mut self) -> Vec<u8> {
        unimplemented!();
    }

    fn seek_block(&mut self, restart_idx: usize) -> Result<(), Error> {
        if restart_idx >= self.block().restarts.len() {
            return Err(Error::Corruption { context: format!("restart_idx={} exceeds num_restarts={}", restart_idx, self.block().restarts.len()) });
        }
        let offset = self.block().restarts[restart_idx] as usize;
        if offset >= self.block().restarts_boundary {
            return Err(Error::Corruption { context: format!("offset={} exceeds restarts_boundary={}", offset, self.block().restarts_boundary) });
        }
        // Parse `key_value`.
        // TODO(rescrv):  It's the third time these 7 lines show up.  Maybe dedupe?
        let mut up = Unpacker::new(&self.block().bytes[offset..self.block().restarts_boundary]);
        let be: BlockEntry = up.unpack()
            .map_err(|e| Error::UnpackError{
                error: e,
                context: format!("could not unpack key-value pair at offset={}", offset),
            })?;
        let next_offset = self.block().restarts_boundary - up.remain().len();
        // Assemble the current cursor.
        let mut key = self.key();
        key.truncate(be.shared());
        key.extend_from_slice(be.key_frag());
        *self = Cursor::Positioned {
            block: self.block(),
            restart_idx: self.restart_idx(),
            offset: offset,
            next_offset: next_offset,
            key,
        };
        Ok(())
    }
}

impl<'a> Iterator for Cursor<'a> {
    fn seek_to_first(&mut self) -> Result<(), super::Error> {
        *self = Cursor::Head {
            block: self.block(),
        };
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), super::Error> {
        *self = Cursor::Tail {
            block: self.block(),
        };
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), super::Error> {
        let mut query = self.clone();
        let mut left: usize = 0usize;
        let mut right: usize = self.block().restarts.len() - 1;

        // Break when left == right.
        while left < right {
            // When left + 1 == right this will set mid == right allowing us to
            // assign right = mid - 1 and hit the left == right condition at top of loop; else,
            // move much closer to the top of the loop.
            let mid = (left + right + 1) / 2;
            query.seek_block(mid)?;
            let value = match query.next()? {
                Some(value) => value,
                None => {
                    right = mid;
                    continue;
                }
            };
            match compare_bytes(key, &value.key) {
                Ordering::Less => {
                    // left     mid     right
                    // |--------|-------|
                    //       |
                    right = mid - 1
                },
                Ordering::Equal => {
                    // left     mid     right
                    // |--------|-------|
                    //          |
                    // When the keys are equal, move left.  We don't move to mid - 1 in case the
                    // first of the key is in mid.
                    right = mid
                },
                Ordering::Greater => {
                    // left     mid     right
                    // |--------|-------|
                    //           |
                    left = mid + 1
                },
            };
        }

        // query has been seek_block'd to the block with the key we are seeking to.
        let mut value = query.same()?;
        while let Some(v) = value {
            if compare_bytes(key, v.key).is_gt() {
                value = query.next()?;
            } else {
                break;
            }
        }

        Ok(())
    }

    fn prev(&mut self) -> Result<Option<KeyValuePair>, super::Error> {
        // This is the offset of the item we will seek to with a call to `next`, so record the
        // offset of next.
        let next_offset = self.next_offset();
        if next_offset <= 0 {
            *self = Cursor::Head {
                block: self.block(),
            };
            return Ok(None)
        }
        // If we happen to be at the boundary of a restart, step.
        let current_restart_idx = self.restart_idx();
        let restart_idx = if next_offset <= self.block().restarts[current_restart_idx] as usize {
            if current_restart_idx == 0 {
                return Err(Error::Corruption {
                    context: format!("next_offset={} <= restarts[{}]={} on zero'th restart",
                                     next_offset, current_restart_idx, self.block().restarts[current_restart_idx]),
                }.into());
            }
            current_restart_idx - 1
        } else {
            current_restart_idx
        };
        if restart_idx >= self.block().restarts_boundary {
            return Err(Error::Corruption {
                context: format!("restart_idx={} exceeds restarts_boundary={}",
                                 restart_idx, self.block().restarts_boundary),
            }.into());
        }
        if restart_idx >= current_restart_idx {
            return Err(Error::Corruption {
                context: format!("restart_idx={} >= previous restart_idx={}",
                                 restart_idx, current_restart_idx),
            }.into());
        }
        // Use next so that we use the same code in both directions.
        self.seek_block(restart_idx)?;
        while self.next_offset() < next_offset {
            self.next()?;
        }
        // TODO(rescrv): Double parsing with same.
        self.same()
    }

    fn next(&mut self) -> Result<Option<KeyValuePair>, super::Error> {
        if let Cursor::Tail { block: _ } = self {
            return Ok(None);
        }
        let offset = self.next_offset();
        if offset >= self.block().restarts_boundary {
            return Ok(None);
        }
        if self.restart_idx() + 1 < self.block().restarts.len() && self.block().restarts[self.restart_idx() + 1] as usize <= offset {
            // We're jumping to the next block, so just seek_block to force a refresh of the key,
            // along with safety checks on key_frag.
            self.seek_block(self.restart_idx() + 1);
            return self.same();
        };
        // Parse `key_value`.
        let mut up = Unpacker::new(&self.block().bytes[offset..self.block().restarts_boundary]);
        let be: BlockEntry = up.unpack()
            .map_err(|e| Error::UnpackError{
                error: e,
                context: format!("could not unpack key-value pair at offset={}", offset),
            })?;
        let next_offset = self.block().restarts_boundary - up.remain().len();
        // Assemble the current cursor.
        let mut key = self.key();
        key.truncate(be.shared());
        key.extend_from_slice(be.key_frag());
        *self = Cursor::Positioned {
            block: self.block(),
            restart_idx: self.restart_idx(),
            offset: offset,
            next_offset: next_offset,
            key,
        };
        // Assemble the return value.
        let kv = KeyValuePair {
            key: match self {
                Cursor::Positioned { block:_, restart_idx: _, offset: _, next_offset: _, key } => { key },
                Cursor::Head { block: _ } => { panic!("we just assigned a Cursor::Positioned to self and it is now a Head") },
                Cursor::Tail { block: _ } => { panic!("we just assigned a Cursor::Positioned to self and it is now a Tail") },
            },
            timestamp: be.timestamp(),
            value: be.value(),
        };
        Ok(Some(kv))
    }

    fn same(&mut self) -> Result<Option<KeyValuePair>, super::Error> {
        let (block, restart_idx, offset, next_offset, key) = match self {
            Cursor::Head { block } => { return Ok(None); }
            Cursor::Tail { block } => { return Ok(None); }
            Cursor::Positioned { block, restart_idx, offset, next_offset, key } => {
                (block, *restart_idx, *offset, *next_offset, key)
            }
        };
        // Parse `key_value`.
        let mut up = Unpacker::new(&block.bytes[offset..block.restarts_boundary]);
        let be: BlockEntry = up.unpack()
            .map_err(|e| Error::UnpackError{
                error: e,
                context: format!("could not unpack key-value pair at offset={}", offset),
            })?;
        // Assemble the return value.
        let kv = KeyValuePair {
            key: match self {
                Cursor::Positioned { block:_, restart_idx: _, offset: _, next_offset: _, key } => { key },
                Cursor::Head { block: _ } => { panic!(format!("we just assigned a Cursor::Positioned to self and it is now a Head")) },
                Cursor::Tail { block: _ } => { panic!(format!("we just assigned a Cursor::Positioned to self and it is now a Tail")) },
            },
            timestamp: be.timestamp(),
            value: be.value(),
        };
        Ok(Some(kv))
    }
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

// Content under CC By-Sa.  I just use as is, as can you.
// https://codereview.stackexchange.com/questions/233872/writing-slice-compare-in-a-more-compact-way
fn compare_bytes(a: &[u8], b: &[u8]) -> cmp::Ordering {
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
    use std::str;

    use super::*;

    #[test]
    fn build_empty_block() {
        let block = Builder::new(BuilderOptions::default());
        let finisher = block.finish();
        let got = finisher.as_slice();
        let exp = &[82, 4, 0, 0, 0, 0, 93, 7, 0, 0, 0];
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
            7, 0, 0, 0,
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
            7, 0, 0, 0,
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

    // TODO(rescrv): Test empty tables.
    // TODO(rescrv): Test corruption cases.
}
