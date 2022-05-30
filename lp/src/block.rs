use std::cmp;

use prototk::{length_free, stack_pack, Message, Packable, Unpacker};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

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
        let pa = stack_pack(length_free(&self.restarts));
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
        let (shared, key_frag) = match be {
            BlockEntry::KeyValuePut(ref x) => (x.shared, x.key_frag),
            BlockEntry::KeyValueDel(ref x) => (x.shared, x.key_frag),
        };

        let pa = stack_pack(be);
        assert!(self.buffer.len() + pa.pack_sz() <= u32::max_value() as usize);
        pa.append_to_vec(&mut self.buffer);

        // Update the last key.
        self.last_key.truncate(shared as usize);
        self.last_key.extend_from_slice(key_frag);

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
        // Number of restarts.
        let mut up = Unpacker::new(&bytes[bytes.len() - 4..]);
        let num_restarts: u32 = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not read last four bytes of block".to_string() })?;
        let num_restarts: usize = num_restarts as usize;
        let restarts_sz = num_restarts * 4 + 4;
        if bytes.len() < restarts_sz {
            return Err(Error::BlockTooSmall { length: bytes.len(), required: restarts_sz })
        }
        let restarts_boundary = bytes.len() - restarts_sz;
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
        let mut block = Block {
            bytes,
            restarts_boundary,
            restarts,
        };
        Ok(block)
    }
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

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
}

impl<'a> Cursor<'a> {
    pub fn new(block: &'a Block) -> Self {
        Cursor {
            block,
            restart_idx: 0,
            offset: 0,
            key_value: None,
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
        };
    }

    pub fn prev(&mut self) -> Result<(), Error> {
        let next_offset = self.offset;
        if next_offset <= 0 {
            self.seek_to_first_tombstone();
            return Ok(());
        }
        let restart_idx = if next_offset <= self.block.restarts[self.restart_idx] as usize {
            // 0th block restart cannot happen here because next_offset == 0 check above.
            self.restart_idx - 1
        } else {
            self.restart_idx
        };
        // We guarantee this unpacker has space because it points to a valid restart point that
        // falls before or at next_offset.
        let mut up = Unpacker::new(&self.block.bytes[self.block.restarts[restart_idx] as usize..next_offset]);
        let mut offset = next_offset - up.remain().len();
        let mut be = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not unpack initial".to_string() })?;
        while !up.empty() {
            offset = next_offset - up.remain().len();
            be = up.unpack()
                .map_err(|e| Error::UnpackError{ error: e, context: "could not unpack loop".to_string() })?;
        }
        *self = Cursor {
            block: self.block,
            restart_idx,
            offset,
            key_value: Some((be, next_offset)),
        };
        Ok(())
    }

    pub fn next(&mut self) -> Result<(), Error> {
        let offset = match (&self.key_value, self.offset >= self.block.restarts_boundary) {
            (_, true) => { self.seek_to_last_tombstone(); self.offset }
            (None, false) => { /* An unprimed state; keep offset */ self.offset }
            (Some((_, off)), false) => { *off }
        };
        // TODO(rescrv):  It would be nice if this would just unpack the next item, but this
        // robustness-tests seek_to_offset by shunting all code through the same paths.
        self.seek_to_offset(offset)
    }

    fn seek_to_offset(&mut self, offset: usize) -> Result<(), Error> {
        if offset >= self.block.restarts_boundary {
            self.seek_to_last_tombstone();
            return Ok(());
        }
        let mut up = Unpacker::new(&self.block.bytes[offset..self.block.restarts_boundary]);
        let be = up.unpack()
            .map_err(|e| Error::UnpackError{ error: e, context: "could not seek to offset".to_string() })?;
        let next_offset = self.block.restarts_boundary - up.remain().len();
        let restart_idx = Self::compute_restart_idx(&self.block, self.restart_idx, offset);
        *self = Cursor {
            block: self.block,
            restart_idx,
            offset,
            key_value: Some((be, next_offset))
        };
        Ok(())
    }

    fn compute_restart_idx(block: &'a Block<'a>, current_restart_index: usize, offset: usize) -> usize {
        // if it is the case that we can probe questions about us and the next restart index.
        if current_restart_index + 1 < block.restarts.len() {
            // If the offset we unpacked falls within the next restart interval, advance.  Can only
            // advance by at most one at a time because there should be no empty restart intervals.
            if block.restarts[current_restart_index + 1] as usize <= offset {
                current_restart_index + 1
            // Otherwise, stay as-is.
            } else {
                current_restart_index
            }
        // Else, we top out.
        } else {
            block.restarts.len() - 1
        }
    }
}

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
        let exp = &[0, 0, 0, 0, 1, 0, 0, 0];
        assert_eq!(exp, got);
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
            0, 0, 0, 0,
            // num_restarts
            1, 0, 0, 0,
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
            66/*8*/, 21/*szXXX*/,
            8/*1*/, 0,
            18/*2*/, 4/*sz*/, 107, 101, 121, 49,
            24/*3*/, /*varint(0xc0ffee)*/238, 255, 131, 6,
            34/*4*/, 6/*sz*/, 118, 97, 108, 117, 101, 49,

            // second record
            66/*8*/, 18/*szXXX*/,
            8/*1*/, 3,
            18/*2*/, 1/*sz*/, 50,
            24/*3*/, /*varint(0xc0ffee)*/238, 255, 131, 6,
            34/*4*/, 6/*sz*/, 118, 97, 108, 117, 101, 50,

            // restarts
            0, 0, 0, 0, 1, 0, 0, 0,
        ];
        assert_eq!(exp, got);
    }

    // TODO(rescrv): Test the restart points code.
    // TODO(rescrv): Test empty tables.
}
