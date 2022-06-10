use std::cmp;
use std::cmp::Ordering;

use prototk::{length_free, stack_pack, Packable, Unpacker, v64};

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
    restarts: Vec<u32>,
}

impl<'a> Block<'a> {
    pub fn initialize<'b: 'a>(bytes: &'b [u8]) -> Result<Self, Error> {
        // Load the restarts.
        if bytes.len() < 4 {
            // This is impossible.  A block must end in a u32 that indicates how many restarts
            // there are.
            return Err(Error::BlockTooSmall { length: bytes.len(), required: 4 })
        }
        let mut up = Unpacker::new(&bytes[bytes.len() - 4..]);
        // Footer size.
        // TODO(rescrv):  ERRORS
        let num_restarts: u32 = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not read last four bytes of block".to_string() })?;
        let restarts_sz = num_restarts as usize * 4;
        let footer_sz: usize = restarts_sz + 2/*tags*/ + 4/*fixed32 cap*/ + v64::from(restarts_sz).pack_sz();
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
            restarts,
        };
        Ok(block)
    }
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

#[derive(Clone)]
struct Cursor<'a> {
    // The block we have a cursor over.  Will always be the same.
    block: &'a Block<'a>,
    // An index [0, num_restarts) that tells us which restart our cursor falls into such that
    // restarts[restarts_idx] <= cursor < restarts[restarts_idx + 1].
    restart_idx: usize,
    // An offset [0, restarts_boundary] of where the cursor is in the file.
    offset: usize,
    // An optional (BlockEntry, next_offset) indicating that the offset has been parsed.  As a
    // special case, can be None when running off the ends of the blocks.
    // INVARIANT: key_value.is_some() || offset == 0 || offset == restarts_boundary.
    key_value: Option<(BlockEntry<'a>, usize)>,
    // The materialized key.
    valid: bool,
    key: Vec<u8>,
}

impl<'a> Cursor<'a> {
    pub fn new(block: &'a Block) -> Self {
        Cursor {
            block,
            restart_idx: 0,
            offset: 0,
            key_value: None,
            key: Vec::new(),
            valid: false,
        }
    }

    pub fn seek_to_first(&mut self) -> Result<(), Error> {
        self.seek_to_first_tombstone();
        self.next()
    }

    pub fn seek_to_first_tombstone(&mut self) {
        *self = Cursor {
            block: self.block,
            restart_idx: 0,
            offset: 0,
            key_value: None,
            key: Vec::new(),
            valid: false,
        };
    }

    pub fn seek_to_last(&mut self) -> Result<(), Error> {
        self.seek_to_last_tombstone();
        self.prev()
    }

    pub fn seek_to_last_tombstone(&mut self) {
        assert!(self.block.restarts.len() > 0);
        let restart_idx = self.block.restarts.len() - 1;
        let offset = self.block.restarts_boundary;
        *self = Cursor {
            block: self.block,
            restart_idx,
            offset,
            key_value: None,
            key: Vec::new(),
            valid: false,
        };
    }

    pub fn seek(&mut self, key: &[u8]) -> Result<(), Error> {
        let mut query = self.clone();
        let mut left: usize = 0usize;
        let mut right: usize = self.block.restarts.len() - 1;

        // Break when left == right.
        while left < right {
            // When left + 1 == right this will set mid == right allowing us to
            // assign right = mid - 1 and hit the left == right condition at top of loop; else,
            // move much closer to the top of the loop.
            let mid = (left + right + 1) / 2;
            query.seek_block(mid)?;
            assert!(query.valid);
            match compare_bytes(key, &query.key) {
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
        while compare_bytes(key, &query.key).is_gt() {
            query.next()?;
        }

        Ok(())
    }

    pub fn prev(&mut self) -> Result<(), Error> {
        // This is the offset of the item we will seek to with a call to `next`, so record the
        // offset of next.
        let next_offset = self.offset;
        if next_offset <= 0 {
            self.seek_to_first_tombstone();
            return Ok(());
        }
        // If we happen to be at the boundary of a restart, step.
        let restart_idx = if next_offset <= self.block.restarts[self.restart_idx] as usize {
            // 0th block restart cannot happen here because next_offset == 0 check above.
            self.restart_idx - 1
        } else {
            self.restart_idx
        };
        // There's a choice here to use an unpacker or to call next().  Calling next() is not that
        // much more expensive, and makes sure to use the correct key materialization code in both
        // directions.  Instead, chose to seek to the restart_idx as the latest-known point where
        // we can pick up and roll forward.  We known from the restart_idx check above that if
        // next_offset is also a restart point, we will select restart_idx as its predecessor and
        // the next_offset will never tangle with the first tombstone.
        self.seek_block(restart_idx)?;
        while let Some((_, next)) = self.key_value {
            if next >= next_offset {
                break;
            }
            self.next()?;
        }
        Ok(())
    }

    pub fn next(&mut self) -> Result<(), Error> {
        // Over-run check.
        if self.offset >= self.block.restarts_boundary {
            self.seek_to_last_tombstone();
            return Ok(());
        }
        // Compute `offset`.
        let offset = match &self.key_value {
            Some((_, next_offset)) => {
                *next_offset
            },
            None => {
                if self.offset == 0 {
                    0
                } else {
                    // There's no case where this should happen, so it shall remain unimplemented.
                    unimplemented!();
                }
            },
        };
        // Compute `restart_idx`.
        let restart_idx = if self.restart_idx + 1 < self.block.restarts.len() && self.block.restarts[self.restart_idx + 1] as usize <= offset {
            self.restart_idx + 1
        } else {
            self.restart_idx
        };
        // Parse `key_value`.
        let mut up = Unpacker::new(&self.block.bytes[offset..self.block.restarts_boundary]);
        let be: BlockEntry = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not unpack next key-value".to_string() })?;
        let next_offset = self.block.restarts_boundary - up.remain().len();
        // Assemble the current cursor.
        self.restart_idx = restart_idx;
        self.offset = offset;
        self.key.truncate(be.shared());
        self.key.extend_from_slice(be.key_frag());
        self.key_value = Some((be, next_offset));
        self.valid = true;
        Ok(())
    }

    fn seek_block(&mut self, restart_idx: usize) -> Result<(), Error> {
        assert!(restart_idx <= self.block.restarts.len());
        // Comput offset.
        let offset = self.block.restarts[restart_idx] as usize;

        // Parse `key_value`.
        let mut up = Unpacker::new(&self.block.bytes[offset..self.block.restarts_boundary]);
        let be: BlockEntry = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not unpack next key-value".to_string() })?;
        let next_offset = self.block.restarts_boundary - up.remain().len();

        // Assemble the key.
        self.key.truncate(0);
        // TODO(rescrv, corruption):  This should always have a shared=0.
        self.key.extend_from_slice(be.key_frag());

        // Assemble the current cursor.
        self.restart_idx = restart_idx;
        self.offset = offset;
        self.key_value = Some((be, next_offset));
        self.valid = true;
        Ok(())
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
    fn prefix_compression() {
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

    // TODO(rescrv): Test the restart points code.
    // TODO(rescrv): Test empty tables.
}
