#![doc = include_str!("../README.md")]

// Copyright (c) 2020 Dropbox, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::convert::TryInto;
use std::fmt::{Debug, Write};

use sha3::{Digest, Sha3_256};

/// The number of bytes in the digest of both the hash used by setsum and the output
/// of setsum.
pub const SETSUM_BYTES: usize = 32;
/// The number of bytes per column.  This should evenly divide the number of bytes.  This number is
/// implicitly wound through the code in its use of u32 to store columns as it's the number of bytes
/// used to store a u32.
const SETSUM_BYTES_PER_COLUMN: usize = 4;
/// The number of columns in the logical/internal representation of the setsum.
const SETSUM_COLUMNS: usize = SETSUM_BYTES / SETSUM_BYTES_PER_COLUMN;
/// Each column uses a different prime to construct a field of different size and transformations.
const SETSUM_PRIMES: [u32; SETSUM_COLUMNS] = [
    4294967291, 4294967279, 4294967231, 4294967197, 4294967189, 4294967161, 4294967143, 4294967111,
];

/// Adds together two internal representations and constructs their output.  The algorithm for
/// column i is `(A[i] + B[i]) % P[i]`, where `P` is the primes array.
#[inline(always)]
pub fn add_state(lhs: [u32; SETSUM_COLUMNS], rhs: [u32; SETSUM_COLUMNS]) -> [u32; SETSUM_COLUMNS] {
    let mut ret = <[u32; SETSUM_COLUMNS]>::default();
    for i in 0..SETSUM_COLUMNS {
        let lc = lhs[i] as u64;
        let rc = rhs[i] as u64;
        let mut sum = lc + rc;
        let p = SETSUM_PRIMES[i] as u64;
        if sum >= p {
            sum -= p;
        }
        ret[i] = sum as u32;
    }
    ret
}

/// Converts each column in the provided state to be the inverse of the input.  This means that the
/// two columns added together via add_state will come out zero.
#[inline(always)]
pub fn invert_state(state: [u32; SETSUM_COLUMNS]) -> [u32; SETSUM_COLUMNS] {
    let mut state = state;
    for i in 0..SETSUM_COLUMNS {
        state[i] = SETSUM_PRIMES[i] - state[i]
    }
    state
}

/// Translate a single hash into the internal representation of a setsum.
fn hash_to_state(hash: &[u8; SETSUM_BYTES]) -> [u32; SETSUM_COLUMNS] {
    let mut item_state = [0u32; SETSUM_COLUMNS];
    for i in 0..SETSUM_COLUMNS {
        let idx = i * SETSUM_BYTES_PER_COLUMN;
        let end = idx + SETSUM_BYTES_PER_COLUMN;
        let buf: [u8; 4] = hash[idx..end].try_into().unwrap();
        let num = u32::from_le_bytes(buf);
        item_state[i] = if num >= SETSUM_PRIMES[i] {
            num - SETSUM_PRIMES[i]
        } else {
            num
        };
    }
    item_state
}

/// Translate an item comprised of multiple vectors to a setsum.
fn item_vectored_to_state(item: &[&[u8]]) -> [u32; SETSUM_COLUMNS] {
    let mut hasher = Sha3_256::default();
    for piece in item {
        hasher.update(piece);
    }
    let mut hash_bytes = hasher.finalize();
    let hash_bytes: &mut [u8; SETSUM_BYTES] = hash_bytes.as_mut();
    hash_to_state(hash_bytes)
}

/// Setsum provides an interactive object for maintaining set checksums.  Technically, multi-set
/// checksums.  Two Setsum objects are equal with high probability if and only if they contain the
/// same items.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Setsum {
    state: [u32; SETSUM_COLUMNS],
}

impl Setsum {
    /// Inserts a new item into the multi-set.  If the item was already inserted, it will be
    /// inserted again.
    pub fn insert(&mut self, item: &[u8]) {
        let item: &[&[u8]] = &[item];
        self.insert_vectored(item);
    }

    /// Vectored version of insert.
    pub fn insert_vectored(&mut self, item: &[&[u8]]) {
        let item_state = item_vectored_to_state(item);
        self.state = add_state(self.state, item_state);
    }

    /// Removes an item from the multi-set.  It is up to the caller to make sure the item already
    /// existed in the multi-set; otherwise, a "placeholder" will be inserted that will consume
    /// one insert of the item.  Multiple placeholders can accrue and all will be removed before the
    /// set matches a set in which the item was inserted.
    pub fn remove(&mut self, item: &[u8]) {
        let item: &[&[u8]] = &[item];
        self.remove_vectored(item);
    }

    /// Vectored version of remove.
    pub fn remove_vectored(&mut self, item: &[&[u8]]) {
        let item_state = item_vectored_to_state(item);
        let item_state = invert_state(item_state);
        self.state = add_state(self.state, item_state);
    }

    /// Computes a byte representation of the setsum for comparison or use in other situations.
    pub fn digest(&self) -> [u8; SETSUM_BYTES] {
        let mut item_hash = [0u8; SETSUM_BYTES];
        for col in 0..SETSUM_COLUMNS {
            let idx = col * SETSUM_BYTES_PER_COLUMN;
            let buf = self.state[col].to_le_bytes();
            item_hash[idx..(4 + idx)].copy_from_slice(&buf[..4]);
        }
        item_hash
    }

    /// Creates a setsum from an array of SETSUM_BYTES.
    pub fn from_digest(digest: [u8; SETSUM_BYTES]) -> Setsum {
        let mut state: [u32; SETSUM_COLUMNS] = [0u32; SETSUM_COLUMNS];
        for (col, item) in state.iter_mut().enumerate().take(SETSUM_COLUMNS) {
            let idx = col * SETSUM_BYTES_PER_COLUMN;
            let mut buf = [0u8; 4];
            buf.clone_from_slice(&digest[idx..idx + 4]);
            *item = u32::from_le_bytes(buf);
        }
        Self { state }
    }

    /// Computes an ASCII/hex representation of setsum for comparison or use in other situations.
    pub fn hexdigest(&self) -> String {
        let mut setsum = String::with_capacity(68);
        let digest = self.digest();
        for item in &digest {
            write!(&mut setsum, "{:02x}", *item).expect("unable to write to string");
        }
        setsum
    }

    /// Creates a setsum from an ASCII/hex string.
    pub fn from_hexdigest(digest: &str) -> Option<Setsum> {
        if digest.len() != SETSUM_BYTES * 2 {
            return None;
        }
        let mut bytes: [u8; SETSUM_BYTES] = [0u8; SETSUM_BYTES];
        for idx in 0..SETSUM_BYTES {
            bytes[idx] = match u8::from_str_radix(&digest[idx * 2..idx * 2 + 2], 16) {
                Ok(b) => b,
                Err(_) => {
                    return None;
                }
            }
        }
        Some(Self::from_digest(bytes))
    }
}

impl Default for Setsum {
    fn default() -> Setsum {
        Setsum {
            state: [0u32; SETSUM_COLUMNS],
        }
    }
}

impl Debug for Setsum {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}", self.hexdigest())
    }
}

impl std::ops::Add<Setsum> for Setsum {
    type Output = Setsum;

    fn add(self, rhs: Setsum) -> Setsum {
        let state = add_state(self.state, rhs.state);
        Setsum { state }
    }
}

impl std::ops::AddAssign<Setsum> for Setsum {
    fn add_assign(&mut self, rhs: Setsum) {
        self.state = add_state(self.state, rhs.state);
    }
}

impl std::ops::Sub<Setsum> for Setsum {
    type Output = Setsum;

    fn sub(self, rhs: Setsum) -> Setsum {
        let rhs_state = invert_state(rhs.state);
        let state = add_state(self.state, rhs_state);
        Setsum { state }
    }
}

impl std::ops::SubAssign<Setsum> for Setsum {
    fn sub_assign(&mut self, rhs: Setsum) {
        let rhs_state = invert_state(rhs.state);
        self.state = add_state(self.state, rhs_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        // require that we're using all the bytes
        assert_eq!(SETSUM_BYTES, SETSUM_BYTES_PER_COLUMN * SETSUM_COLUMNS);
    }

    #[test]
    fn add_state_no_modulus() {
        let lhs: [u32; SETSUM_COLUMNS] = [1, 2, 3, 4, 5, 6, 7, 8];
        let rhs: [u32; SETSUM_COLUMNS] = [2, 4, 6, 8, 10, 12, 14, 16];
        let expected: [u32; SETSUM_COLUMNS] = [3, 6, 9, 12, 15, 18, 21, 24];
        let returned: [u32; SETSUM_COLUMNS] = add_state(lhs, rhs);
        assert_eq!(expected, returned);
    }

    #[test]
    fn add_state_exactly_primes() {
        let lhs: [u32; SETSUM_COLUMNS] = [
            3146800025, 1792545563, 417324692, 3444237760, 2812742746, 1608771649, 1661742866,
            3220115897,
        ];
        let rhs: [u32; SETSUM_COLUMNS] = [
            1148167266, 2502421716, 3877642539, 850729437, 1482224443, 2686195512, 2633224277,
            1074851214,
        ];
        let expected = [0u32; SETSUM_COLUMNS];
        let returned = add_state(lhs, rhs);
        assert_eq!(expected, returned);
    }

    #[test]
    fn invert_state_desc() {
        let state_in: [u32; SETSUM_COLUMNS] = [
            0xffffeeee, 0xddddcccc, 0xbbbbaaaa, 0x99998888, 0x77776666, 0x66665555, 0x44443333,
            0x22221111,
        ];
        let expected: [u32; SETSUM_COLUMNS] = [
            4365, 572666659, 1145328917, 1717991189, 2290653487, 2576984612, 3149646900, 3722309174,
        ];
        let returned = invert_state(state_in);
        assert_eq!(expected, returned);
    }

    fn hash_to_state_visual_helper(x: u32, buf: &mut [u8]) {
        assert!(buf.len() >= 4);
        let arr = x.to_le_bytes();
        buf[..4].copy_from_slice(&arr[..4]);
    }

    #[test]
    fn hash_to_state_visual() {
        let primes: [u32; SETSUM_COLUMNS] = [2, 3, 5, 7, 11, 13, 17, 19];
        let mut hash = [0u8; 32];
        for (i, prime) in primes.iter().enumerate() {
            let idx = i * SETSUM_BYTES_PER_COLUMN;
            hash_to_state_visual_helper(*prime, &mut hash[idx..]);
        }
        let state = hash_to_state(&hash);
        assert_eq!(primes, state);
    }

    #[test]
    fn empty_item_to_state() {
        let expected: [u32; SETSUM_COLUMNS] = [
            0xf8c6ffa7, 0x66d71ebf, 0x5647c151, 0x62d661a0, 0x4dff80f5, 0xfa493be4, 0x4b0ad882,
            0x4a43f880,
        ];
        let returned: [u32; SETSUM_COLUMNS] = item_vectored_to_state(&[]);
        assert_eq!(expected, returned)
    }

    // This was chosen by running the _sorted variety, so the strength of that test is weakened.
    // For the remainder, it is an expected value chosen from outside of the test.
    const SEVEN_VALUES: [u8; 32] = [
        197, 179, 253, 77, 1, 242, 184, 4, 15, 84, 171, 116, 18, 202, 83, 187, 252, 153, 14, 39,
        42, 64, 173, 209, 196, 206, 186, 107, 47, 228, 114, 213,
    ];

    #[test]
    fn setsum_insert_7_sorted() {
        let mut setsum = Setsum::default();
        setsum.insert(b"this is the first value");
        setsum.insert(b"this is the second value");
        setsum.insert(b"this is the third value");
        setsum.insert(b"this is the fourth value");
        setsum.insert(b"this is the fifth value");
        setsum.insert(b"this is the sixth value");
        setsum.insert(b"this is the seventh value");
        let digest = setsum.digest();
        assert_eq!(SEVEN_VALUES, digest);
    }

    #[test]
    fn setsum_insert_7_reversed() {
        let mut setsum = Setsum::default();
        setsum.insert(b"this is the seventh value");
        setsum.insert(b"this is the sixth value");
        setsum.insert(b"this is the fifth value");
        setsum.insert(b"this is the fourth value");
        setsum.insert(b"this is the third value");
        setsum.insert(b"this is the second value");
        setsum.insert(b"this is the first value");
        let digest = setsum.digest();
        assert_eq!(SEVEN_VALUES, digest);
    }

    #[test]
    fn setsum_insert_7_random() {
        let mut setsum = Setsum::default();
        setsum.insert(b"this is the fifth value");
        setsum.insert(b"this is the fourth value");
        setsum.insert(b"this is the third value");
        setsum.insert(b"this is the sixth value");
        setsum.insert(b"this is the seventh value");
        setsum.insert(b"this is the second value");
        setsum.insert(b"this is the first value");
        let digest = setsum.digest();
        assert_eq!(SEVEN_VALUES, digest);
    }

    #[test]
    fn setsum_insert_remove() {
        let mut setsum = Setsum::default();
        setsum.insert(b"this is the first value");
        setsum.insert(b"this is the second value");
        setsum.insert(b"this is the third value");
        setsum.insert(b"this is the fourth value");
        setsum.insert(b"this is the fifth value");
        setsum.insert(b"this is the sixth value");
        setsum.insert(b"this is the seventh value");
        setsum.remove(b"this is the seventh value");
        setsum.remove(b"this is the sixth value");
        setsum.remove(b"this is the fifth value");
        setsum.remove(b"this is the fourth value");
        setsum.remove(b"this is the third value");
        setsum.remove(b"this is the second value");
        setsum.remove(b"this is the first value");
        let digest = setsum.digest();
        assert_eq!(Setsum::default().digest(), digest);
    }

    #[test]
    fn setsum_merge_two_sets() {
        let mut setsum_one = Setsum::default();
        setsum_one.insert(b"this is the first value");
        setsum_one.insert(b"this is the second value");
        setsum_one.insert(b"this is the third value");
        setsum_one.insert(b"this is the fourth value");

        let mut setsum_two = Setsum::default();
        setsum_two.insert(b"this is the fifth value");
        setsum_two.insert(b"this is the sixth value");
        setsum_two.insert(b"this is the seventh value");

        let setsum_one_plus_two = setsum_one + setsum_two;
        let digest = setsum_one_plus_two.digest();
        assert_eq!(SEVEN_VALUES, digest);
    }

    #[test]
    fn setsum_remove_two_sets() {
        let mut setsum = Setsum::default();
        setsum.insert(b"this is the first value");
        setsum.insert(b"this is the second value");
        setsum.insert(b"this is the third value");
        setsum.insert(b"this is the fourth value");
        setsum.insert(b"this is the fifth value");
        setsum.insert(b"this is the sixth value");
        setsum.insert(b"this is the seventh value");

        let mut setsum_one = Setsum::default();
        setsum_one.insert(b"this is the first value");
        setsum_one.insert(b"this is the second value");
        setsum_one.insert(b"this is the third value");
        setsum_one.insert(b"this is the fourth value");

        let mut setsum_two = Setsum::default();
        setsum_two.insert(b"this is the fifth value");
        setsum_two.insert(b"this is the sixth value");
        setsum_two.insert(b"this is the seventh value");

        let setsum_empty = setsum - setsum_one - setsum_two;
        let digest = setsum_empty.digest();
        assert_eq!(Setsum::default().digest(), digest);
    }

    #[test]
    fn setsum_from_digest() {
        let mut setsum = Setsum::default();
        setsum.insert(b"this is the first value");
        setsum.insert(b"this is the second value");
        setsum.insert(b"this is the third value");
        setsum.insert(b"this is the fourth value");
        setsum.insert(b"this is the fifth value");
        setsum.insert(b"this is the sixth value");
        setsum.insert(b"this is the seventh value");
        assert_eq!(Setsum::from_digest(SEVEN_VALUES), setsum);
    }

    #[test]
    fn setsum_from_hexdigest() {
        const SEVEN_HEX_VALUES: &str =
            "c5b3fd4d01f2b8040f54ab7412ca53bbfc990e272a40add1c4ceba6b2fe472d5";
        assert_eq!(
            Setsum::from_digest(SEVEN_VALUES),
            Setsum::from_hexdigest(SEVEN_HEX_VALUES).unwrap()
        );
    }
}
