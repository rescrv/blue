use std::cmp;

use prototk::{length_free, stack_pack, Message, Packable, Unpacker};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

pub enum Error {
    BlockTooSmall{ length: usize, required: usize },
    UnpackError{ error: prototk::Error, context: String },
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
        Builder {
            options,
            buffer,
            last_key: Vec::default(),
            restarts: Vec::default(),
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
        let mut block = Block {
            bytes,
            restarts_boundary,
            restarts,
        };
        Ok(block)
    }
}
