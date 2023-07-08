//! [Guacamole] provides a linearly-seekable random number generator.
//! [Zipf] provides a zipf-distribution sampler.

extern crate rand;

use rand::RngCore;

pub mod zipf;

pub use zipf::Zipf;

/////////////////////////////////////////////// mash ///////////////////////////////////////////////

/*
Derived from salsa208, but chopped up for speed.
Guacamole is a better dip for chips than Salsa.
This is a straight port from C:  https://github.com/rescrv/ygor/blob/master/guacamole.cc#L277

Code was released under:
version 20080913
D. J. Bernstein
Public domain.
*/

fn mash(x: u64, output: &mut [u32; 16]) {
    let low: u32 = (x & 0xffffffffu64) as u32;
    let high: u32 = (x >> 32) as u32;

    let mut x0: u32 = 1634760805;
    let mut x1: u32 = 0;
    let mut x2: u32 = 0;
    let mut x3: u32 = 0;
    let mut x4: u32 = 0;
    let mut x5: u32 = 857760878;
    let mut x6: u32 = low;
    let mut x7: u32 = high;
    let mut x8: u32 = 0;
    let mut x9: u32 = 0;
    let mut x10: u32 = 2036477234;
    let mut x11: u32 = 0;
    let mut x12: u32 = 0;
    let mut x13: u32 = 0;
    let mut x14: u32 = 0;
    let mut x15: u32 = 1797285236;

    for _ in 0..4 {
        let tmp = x8.wrapping_add(x12);
        x4 ^= tmp.rotate_left(7);
        let tmp = x13.wrapping_add(x1);
        x9 ^= tmp.rotate_left(7);
        let tmp = x2.wrapping_add(x6);
        x14 ^= tmp.rotate_left(7);
        let tmp = x7.wrapping_add(x11);
        x3 ^= tmp.rotate_left(7);
        let tmp = x12.wrapping_add(x0);
        x8 ^= tmp.rotate_left(9);
        let tmp = x1.wrapping_add(x5);
        x13 ^= tmp.rotate_left(9);
        let tmp = x6.wrapping_add(x10);
        x2 ^= tmp.rotate_left(9);
        let tmp = x11.wrapping_add(x15);
        x7 ^= tmp.rotate_left(9);
        let tmp = x0.wrapping_add(x4);
        x12 ^= tmp.rotate_left(13);
        let tmp = x5.wrapping_add(x9);
        x1 ^= tmp.rotate_left(13);
        let tmp = x10.wrapping_add(x14);
        x6 ^= tmp.rotate_left(13);
        let tmp = x15.wrapping_add(x3);
        x11 ^= tmp.rotate_left(13);
        let tmp = x4.wrapping_add(x8);
        x0 ^= tmp.rotate_left(18);
        let tmp = x9.wrapping_add(x13);
        x5 ^= tmp.rotate_left(18);
        let tmp = x14.wrapping_add(x2);
        x10 ^= tmp.rotate_left(18);
        let tmp = x3.wrapping_add(x7);
        x15 ^= tmp.rotate_left(18);
        let tmp = x2.wrapping_add(x3);
        x1 ^= tmp.rotate_left(7);
        let tmp = x7.wrapping_add(x4);
        x6 ^= tmp.rotate_left(7);
        let tmp = x8.wrapping_add(x9);
        x11 ^= tmp.rotate_left(7);
        let tmp = x13.wrapping_add(x14);
        x12 ^= tmp.rotate_left(7);
        let tmp = x3.wrapping_add(x0);
        x2 ^= tmp.rotate_left(9);
        let tmp = x4.wrapping_add(x5);
        x7 ^= tmp.rotate_left(9);
        let tmp = x9.wrapping_add(x10);
        x8 ^= tmp.rotate_left(9);
        let tmp = x14.wrapping_add(x15);
        x13 ^= tmp.rotate_left(9);
        let tmp = x0.wrapping_add(x1);
        x3 ^= tmp.rotate_left(13);
        let tmp = x5.wrapping_add(x6);
        x4 ^= tmp.rotate_left(13);
        let tmp = x10.wrapping_add(x11);
        x9 ^= tmp.rotate_left(13);
        let tmp = x15.wrapping_add(x12);
        x14 ^= tmp.rotate_left(13);
        let tmp = x1.wrapping_add(x2);
        x0 ^= tmp.rotate_left(18);
        let tmp = x6.wrapping_add(x7);
        x5 ^= tmp.rotate_left(18);
        let tmp = x11.wrapping_add(x8);
        x10 ^= tmp.rotate_left(18);
        let tmp = x12.wrapping_add(x13);
        x15 ^= tmp.rotate_left(18);
    }

    let x0 = x0.wrapping_add(1634760805);
    let x5 = x5.wrapping_add(857760878);
    let x6 = x6.wrapping_add(low);
    let x7 = x7.wrapping_add(high);
    let x10 = x10.wrapping_add(2036477234);
    let x15 = x15.wrapping_add(1797285236);

    output[0] = x4;
    output[1] = x9;
    output[2] = x14;
    output[3] = x3;
    output[4] = x8;
    output[5] = x13;
    output[6] = x2;
    output[7] = x7;
    output[8] = x12;
    output[9] = x1;
    output[10] = x6;
    output[11] = x11;
    output[12] = x0;
    output[13] = x5;
    output[14] = x10;
    output[15] = x15;
}

///////////////////////////////////////////// Guacamole ////////////////////////////////////////////

#[repr(C)]
#[derive(Clone, Copy)]
union MashOutput {
    bytes: [u8; 64],
    blocks: [u32; 16],
}

impl MashOutput {
    fn as_bytes(&mut self) -> &mut [u8; 64] {
        unsafe { &mut self.bytes }
    }

    fn as_blocks(&mut self) -> &mut [u32; 16] {
        unsafe { &mut self.blocks }
    }
}

/// Guacamole is a linearly-seekable random number generator.  The linearly-seekable property comes
/// from the fact that the seed for the random number generator preserves spacing in the input.
/// Each one unit increase in the seed corresponds to a 64B movement in the output.  This allows
/// workloads that are partitionable or discrete with many members to predictably manipulate the
/// output by the input.
#[derive(Clone)]
pub struct Guacamole {
    // we treat this like the nonce in the stream cipher
    nonce: u64,
    // index of bytes generated within the current nonce [0, 64).
    index: usize,
    // buffer of generated bytes
    buffer: MashOutput,
}

impl Guacamole {
    /// Create a new [Guacamole] and seek it to `x`.
    pub fn new(x: u64) -> Self {
        let mut g = Guacamole::default();
        g.seek(x);
        g
    }

    /// Seek to `x`.  The seek space is over 64-bits for efficiency, while the output of guacamole is
    /// over 70-bits.  Each seek `i` to `i+1` advances 64 bytes in the output stream.
    pub fn seek(&mut self, x: u64) {
        self.nonce = x;
        self.index = 0;
        // TODO(rescrv): find a way out of this unsafe block (and the other)
        mash(self.nonce, self.buffer.as_blocks());
    }

    /// Fill `bytes` with the next `bytes.len()` random bytes from the stream.
    pub fn generate(&mut self, bytes: &mut [u8]) {
        let mut bytes = bytes;
        while bytes.len() >= self.remaining_len() {
            let rem = self.remaining_len();
            bytes[..rem].copy_from_slice(&self.buffer.as_bytes()[self.index..]);
            bytes = &mut bytes[rem..];
            let (nonce, _) = self.nonce.overflowing_add(1);
            self.seek(nonce);
        }
        assert!(bytes.len() < self.remaining_len());
        let rem = bytes.len();
        bytes[..rem].copy_from_slice(&self.buffer.as_bytes()[self.index..self.index + rem]);
        self.index += rem;
        assert!(self.index < 64);
    }

    fn remaining_len(&self) -> usize {
        assert!(self.index <= 64);
        64 - self.index
    }
}

impl Default for Guacamole {
    /// Returns a Guacamole that has been seek'd to 0.
    fn default() -> Self {
        let mut g = Guacamole {
            nonce: 0,
            index: 0,
            buffer: MashOutput { blocks: [0; 16] },
        };
        // Need to seek to mash the buffer.
        g.seek(0);
        g
    }
}

impl RngCore for Guacamole {
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        Guacamole::generate(self, buf)
    }

    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn try_fill_bytes(&mut self, buf: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(buf);
        Ok(())
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    use rand::Rng;

    const TEST_CASES: &[(&'static str, u64, [u32; 16])] = &[
        (
            "zero",
            0,
            [
                0x4f59ed0c, 0xe64b19b6, 0xf18f8d1c, 0x253596d5, 0x84070408, 0x38cb6aad, 0x29f6d1cd,
                0xb4b7da12, 0xbd71b078, 0xc02abbe, 0x2cadf87a, 0xbeb2667f, 0x23adb35, 0xf3bf9552,
                0xf038a8ec, 0x5744f030,
            ],
        ),
        (
            "coffee",
            0xc0ffee,
            [
                0x678d8948, 0x4f3aecd0, 0x1e4f3497, 0x99dce441, 0xcd4636d4, 0x29ca9bd6, 0x7c79ff15,
                0x4ed18966, 0xe1e1f2e0, 0x2404b81, 0x9cd9ea9e, 0xc17e6003, 0x97889ecb, 0x294cdc42,
                0x5199ae4d, 0x4db39d1,
            ],
        ),
        (
            "food",
            0xf00d,
            [
                0xca31e7e1, 0x59a2177a, 0xf549c8d4, 0xe6e1b4a7, 0x5484c5c9, 0x6079a2f, 0x596d080,
                0x43dea1d0, 0x6416d5c1, 0x7782d182, 0x6576f421, 0xf5ebd87c, 0x5ac41233, 0x75555efd,
                0x6a9ffb5e, 0xf1a53100,
            ],
        ),
        (
            "uint64_max",
            0xffffffffffffffffu64,
            [
                0x6af23cfe, 0x2d9f7eb6, 0x931ecca1, 0xa42d5caf, 0x7f8c5ab7, 0x2be0cef9, 0xbcd91977,
                0x9d85c40a, 0xcd638380, 0xc97e931a, 0x160327ca, 0xa954360a, 0x83887bcf, 0x2c680a44,
                0xaf593192, 0x3a29d573,
            ],
        ),
    ];

    #[test]
    fn mash() {
        for (descr, seed, output) in TEST_CASES {
            let buffer = &mut [0u32; 16];
            crate::mash(*seed, buffer);
            assert_eq!(output, buffer, "test case = {}", descr);
        }
    }

    #[test]
    fn guacamole() {
        for (descr, seed, output) in TEST_CASES {
            let mut g = Guacamole::default();
            g.seek(*seed);
            for i in 0..16 {
                let x: u32 = g.gen::<u32>();
                assert_eq!(output[i], x, "test case = {}[{}]", descr, i);
            }
        }
    }
}
