//! This file implements the split-block bloom-filter used by parquet:
//! https://github.com/apache/parquet-format/blob/master/BloomFilter.md
use std::convert::TryFrom;

use siphasher::sip::SipHasher24;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

const SALT: [u32; 8] = [
    0x47b6137bu32,
    0x44974d91u32,
    0x8824ad5bu32,
    0xa2b7289du32,
    0x705495c7u32,
    0x2df1424bu32,
    0x9efc4947u32,
    0x5c6bfb31u32,
];

const KEY: [u8; 16] = [
    98, 124, 9, 13, 179, 65, 108, 38, 187, 225, 14, 208, 137, 80, 122, 145,
];

/////////////////////////////////////////////// Block //////////////////////////////////////////////

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct Block {
    block: [u32; 8],
}

impl Block {
    fn mask(x: u32) -> Block {
        let mut result = Block::default();
        #[allow(clippy::needless_range_loop)]
        for i in 0..8 {
            let y: u32 = (x as u64 * SALT[i] as u64) as u32;
            result.block[i] |= 1 << (y >> 27);
        }
        result
    }

    fn insert(&mut self, x: u32) {
        let mask = Block::mask(x);
        for i in 0..8 {
            self.block[i] |= mask.block[i];
        }
    }

    fn check(&self, x: u32) -> bool {
        let mask = Block::mask(x);
        for i in 0..8 {
            if self.block[i] & mask.block[i] != mask.block[i] {
                return false;
            }
        }
        true
    }

    fn append_to_bytes(&self, buf: &mut Vec<u8>) {
        for b in self.block.iter() {
            buf.extend_from_slice(&b.to_le_bytes());
        }
    }
}

impl TryFrom<&[u8]> for Block {
    type Error = &'static str;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 32 {
            return Err("block must be exactly 32 bytes");
        }
        let mut block = Block::default();
        for i in 0..8 {
            let mut one: [u8; 4] = [0; 4];
            let idx = i * 4;
            one.copy_from_slice(&bytes[idx..idx + 4]);
            block.block[i] = u32::from_le_bytes(one);
        }
        Ok(block)
    }
}

////////////////////////////////////////////// Filter //////////////////////////////////////////////

/// A split-block bloom-filter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Filter {
    blocks: Vec<Block>,
}

impl Filter {
    /// Create a new bloom filter with size bits.
    pub fn new(size: u32) -> Self {
        // Add 7 and divide by 8 to get number of bytes.
        // Divide by 32 for number of blocks.
        // Add one to make sure it's never 0.
        let size = ((size.saturating_add(7) >> 3) >> 5) + 1;
        assert!(size > 0);
        let blocks = vec![Block::default(); size.try_into().unwrap()];
        Self { blocks }
    }

    /// Approximate size of the filter, not including the struct itself.
    pub fn approximate_size(&self) -> usize {
        self.blocks.len() * std::mem::size_of::<Block>()
    }

    /// Insert item into the bloom filter.
    pub fn insert(&mut self, item: &[u8]) {
        self.deferred_insert(Self::defer_insert(item));
    }

    /// Check if item exists in the bloom filter.
    pub fn check(&self, item: &[u8]) -> bool {
        let x = Self::defer_insert(item);
        let (block_idx, x) = self.do_hashing(x);
        self.blocks[block_idx].check(x)
    }

    /// Convert a bloom filter into a byte string.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32 * self.blocks.len());
        for b in self.blocks.iter() {
            b.append_to_bytes(&mut bytes);
        }
        bytes
    }

    /// Defer inserting this item; instead, return a u64 that can be provided to a subsequent
    /// [deferred_insert].
    pub fn defer_insert(item: &[u8]) -> u64 {
        let hasher = SipHasher24::new_with_key(&KEY);
        hasher.hash(item)
    }

    /// Insert an item previously returned by [defer_insert].
    pub fn deferred_insert(&mut self, item: u64) {
        let (block_idx, x) = self.do_hashing(item);
        self.blocks[block_idx].insert(x);
    }

    fn do_hashing(&self, x: u64) -> (usize, u32) {
        let block_idx = (((x >> 32) * self.blocks.len() as u64) >> 32) as usize;
        assert!(
            block_idx < self.blocks.len(),
            "this is ensured by how we compute block_idx"
        );
        (block_idx, x as u32)
    }
}

impl TryFrom<&[u8]> for Filter {
    type Error = &'static str;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.is_empty() {
            return Err("bloom filter must have a non-zero length");
        }
        if !bytes.len().is_multiple_of(32) {
            return Err("bloom filter must be a multiple of 32 in length");
        }
        let limit = bytes.len() / 32;
        let mut filter = Filter {
            blocks: Vec::with_capacity(limit),
        };
        for idx in 0..limit {
            let idx = idx * 32;
            let block_bytes = &bytes[idx..idx + 32];
            let block = Block::try_from(block_bytes)?;
            filter.blocks.push(block);
        }
        Ok(filter)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::Filter;

    #[test]
    fn filter() {
        let mut filter = Filter::new(128);
        assert!(!filter.check(b"hello world"));
        filter.insert(b"hello world");
        assert!(filter.check(b"hello world"));
        let bytes: &[u8] = &filter.to_bytes();
        let filter2: Filter = Filter::try_from(bytes).expect("try_from should succeed");
        assert_eq!(filter, filter2);
    }
}
