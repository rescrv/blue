#![doc = include_str!("../README.md")]

pub mod combinators;

mod zipf;

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
        let mut g = Guacamole {
            nonce: 0,
            index: 0,
            buffer: MashOutput { blocks: [0; 16] },
        };
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

/////////////////////////////////////////// FromGuacamole //////////////////////////////////////////

/// Generate Self from a reference to T and an instance of guacamole.
///
/// See the combinators module for this put to good effect.
pub trait FromGuacamole<T> {
    fn from_guacamole(t: &mut T, guac: &mut Guacamole) -> Self;
}

///////////////////////////////////////////// integers /////////////////////////////////////////////

macro_rules! guacamole_from_le_bytes {
    ($what:ty) => {
        impl FromGuacamole<()> for $what {
            fn from_guacamole(_: &mut (), guac: &mut Guacamole) -> Self {
                const SZ: usize = std::mem::size_of::<$what>();
                let mut buf: [u8; SZ] = [0; SZ];
                guac.generate(&mut buf);
                <$what>::from_le_bytes(buf)
            }
        }
    };
}

guacamole_from_le_bytes!(i8);
guacamole_from_le_bytes!(u8);

guacamole_from_le_bytes!(i16);
guacamole_from_le_bytes!(u16);

guacamole_from_le_bytes!(i32);
guacamole_from_le_bytes!(u32);

guacamole_from_le_bytes!(i64);
guacamole_from_le_bytes!(u64);

guacamole_from_le_bytes!(i128);
guacamole_from_le_bytes!(u128);

guacamole_from_le_bytes!(isize);
guacamole_from_le_bytes!(usize);

////////////////////////////////////////// floating point //////////////////////////////////////////

// Trick confirmed by:
// https://prng.di.unimi.it/

impl FromGuacamole<()> for f32 {
    fn from_guacamole(_: &mut (), guac: &mut Guacamole) -> Self {
        let mut buf = [0u8; 4];
        guac.generate(&mut buf[0..3]);
        let x = u32::from_le_bytes(buf);
        (x & 0xffffffu32) as f32 / (1u32 << f32::MANTISSA_DIGITS) as f32
    }
}

impl FromGuacamole<()> for f64 {
    fn from_guacamole(_: &mut (), guac: &mut Guacamole) -> Self {
        let mut buf = [0u8; 8];
        guac.generate(&mut buf[0..7]);
        let x = u64::from_le_bytes(buf);
        (x & 0x1fffffffffffffu64) as f64 / (1u64 << f64::MANTISSA_DIGITS) as f64
    }
}

/////////////////////////////////////////////// tuple //////////////////////////////////////////////

macro_rules! guacamole_from_tuple {
    ( $($name:ident)+ ) => {
        #[allow(non_snake_case)]
        impl<$($name: FromGuacamole<()>),+> FromGuacamole<()> for ($($name,)+) {
            fn from_guacamole(_: &mut (), guac: &mut Guacamole) -> Self {
                $(let $name = $name::from_guacamole(&mut (), guac);)+
                ($($name,)+)
            }
        }
    };
}

guacamole_from_tuple! { A }
guacamole_from_tuple! { A B }
guacamole_from_tuple! { A B C }
guacamole_from_tuple! { A B C D }
guacamole_from_tuple! { A B C D E }
guacamole_from_tuple! { A B C D E F }
guacamole_from_tuple! { A B C D E F G }
guacamole_from_tuple! { A B C D E F G H }
guacamole_from_tuple! { A B C D E F G H I }
guacamole_from_tuple! { A B C D E F G H I J }
guacamole_from_tuple! { A B C D E F G H I J K }
guacamole_from_tuple! { A B C D E F G H I J K L }
guacamole_from_tuple! { A B C D E F G H I J K L M }
guacamole_from_tuple! { A B C D E F G H I J K L M N }
guacamole_from_tuple! { A B C D E F G H I J K L M N O }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S T }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S T U }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S T U V }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W X }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W X Y }
guacamole_from_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W X Y Z }

/////////////////////////////////////////////// array //////////////////////////////////////////////

macro_rules! guacamole_from_array {
    ( $num:tt $($name:ident)+ ) => {
        #[allow(non_snake_case)]
        impl<U, T: FromGuacamole<U>> FromGuacamole<U> for [T; $num] {
            fn from_guacamole(u: &mut U, guac: &mut Guacamole) -> Self {
                $(let $name = T::from_guacamole(u, guac);)+
                [$($name,)+]
            }
        }
    };
}

guacamole_from_array! { 1 A }
guacamole_from_array! { 2 A B }
guacamole_from_array! { 3 A B C }
guacamole_from_array! { 4 A B C D }
guacamole_from_array! { 5 A B C D E }
guacamole_from_array! { 6 A B C D E F }
guacamole_from_array! { 7 A B C D E F G }
guacamole_from_array! { 8 A B C D E F G H }
guacamole_from_array! { 9 A B C D E F G H I }
guacamole_from_array! { 10 A B C D E F G H I J }
guacamole_from_array! { 11 A B C D E F G H I J K }
guacamole_from_array! { 12 A B C D E F G H I J K L }
guacamole_from_array! { 13 A B C D E F G H I J K L M }
guacamole_from_array! { 14 A B C D E F G H I J K L M N }
guacamole_from_array! { 15 A B C D E F G H I J K L M N O }
guacamole_from_array! { 16 A B C D E F G H I J K L M N O P }
guacamole_from_array! { 17 A B C D E F G H I J K L M N O P Q }
guacamole_from_array! { 18 A B C D E F G H I J K L M N O P Q R }
guacamole_from_array! { 19 A B C D E F G H I J K L M N O P Q R S }
guacamole_from_array! { 20 A B C D E F G H I J K L M N O P Q R S T }
guacamole_from_array! { 21 A B C D E F G H I J K L M N O P Q R S T U }
guacamole_from_array! { 22 A B C D E F G H I J K L M N O P Q R S T U V }
guacamole_from_array! { 23 A B C D E F G H I J K L M N O P Q R S T U V W }
guacamole_from_array! { 24 A B C D E F G H I J K L M N O P Q R S T U V W X }
guacamole_from_array! { 25 A B C D E F G H I J K L M N O P Q R S T U V W X Y }
guacamole_from_array! { 26 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z }
guacamole_from_array! { 27 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z AA }
guacamole_from_array! { 28 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z AA AB }
guacamole_from_array! { 29 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z AA AB AC }
guacamole_from_array! { 30 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z AA AB AC AD }
guacamole_from_array! { 31 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z AA AB AC AD AE }
guacamole_from_array! { 32 A B C D E F G H I J K L M N O P Q R S T U V W X Y Z AA AB AC AD AE AF }

/////////////////////////////////////////////// char ///////////////////////////////////////////////

// From the rust documentation:
//
// A char is a ‘Unicode scalar value’, which is any ‘Unicode code point’ other than a surrogate
// code point. This has a fixed numerical definition: code points are in the range 0 to 0x10FFFF,
// inclusive. Surrogate code points, used by UTF-16, are in the range 0xD800 to 0xDFFF.
const MAX_CHAR: u32 = 0x10FFFFu32;
const FIRST_SURROGATE: u32 = 0xD800u32;
const LAST_SURROGATE: u32 = 0xDFFFu32;
const SURROGATE_GAP: u32 = LAST_SURROGATE - FIRST_SURROGATE + 1;
const CHAR_RANGE: u32 = MAX_CHAR - SURROGATE_GAP + 1;

fn char_from_u24(x: u32) -> char {
    let c = ((x as u64 * CHAR_RANGE as u64) >> 24) as u32;
    if c >= FIRST_SURROGATE {
        char::from_u32(c + SURROGATE_GAP).expect("char should be utf8")
    } else {
        char::from_u32(c).expect("char should be utf8")
    }
}

impl FromGuacamole<()> for char {
    fn from_guacamole(_: &mut (), guac: &mut Guacamole) -> Self {
        let mut buf = [0u8; 4];
        guac.generate(&mut buf[0..3]);
        let x = u32::from_le_bytes(buf);
        char_from_u24(x)
    }
}

////////////////////////////////////////// weighted macro //////////////////////////////////////////

/// Run a block of code with the weight assigned to it.  For example:
///
/// ```
/// use guacamole::FromGuacamole;
///
/// #[derive(Clone, Debug, Eq, PartialEq)]
/// enum Count {
///     One,
///     Two,
///     Three,
/// }
///
/// let func = guacamole::weighted! {
///     0.5 => {
///         Count::One
///     }
///     0.25 => {
///         Count::Two
///     }
///     0.25 => {
///         Count::Three
///     }
/// };
/// ```
#[macro_export]
macro_rules! weighted {
    ($($weight:literal => $code:block)+) => {
        {
            let total = $($weight + )+ 0.0;
            let acc = 0.0;
            move |guac: &mut $crate::Guacamole| {
                let weight = f32::from_guacamole(&mut (), guac) * total;
                $(
                    let acc = acc + $weight;
                    if weight <= acc {
                        return {
                            $code
                        }
                    }
                )+
                panic!("error calculating weighted probabilities");
            }
        }
    };
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CASES: &[(&str, u64, [u32; 16])] = &[
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
            for (i, item) in output.iter().enumerate().take(16) {
                let x: u32 = u32::from_guacamole(&mut (), &mut g);
                assert_eq!(*item, x, "test case = {}[{}]", descr, i);
            }
        }
    }

    #[test]
    fn i8_consts() {
        let mut g = Guacamole::default();
        assert_eq!(12i8, i8::from_guacamole(&mut (), &mut g));
        assert_eq!(-19i8, i8::from_guacamole(&mut (), &mut g));
        assert_eq!(89i8, i8::from_guacamole(&mut (), &mut g));
        assert_eq!(79i8, i8::from_guacamole(&mut (), &mut g));
    }

    #[test]
    fn u8_consts() {
        let mut g = Guacamole::default();
        assert_eq!(12u8, u8::from_guacamole(&mut (), &mut g));
        assert_eq!(237u8, u8::from_guacamole(&mut (), &mut g));
        assert_eq!(89u8, u8::from_guacamole(&mut (), &mut g));
        assert_eq!(79u8, u8::from_guacamole(&mut (), &mut g));
    }

    #[test]
    fn i16_consts() {
        let mut g = Guacamole::default();
        assert_eq!(-4852i16, i16::from_guacamole(&mut (), &mut g));
        assert_eq!(20313i16, i16::from_guacamole(&mut (), &mut g));
        assert_eq!(6582i16, i16::from_guacamole(&mut (), &mut g));
        assert_eq!(-6581i16, i16::from_guacamole(&mut (), &mut g));
    }

    #[test]
    fn u16_consts() {
        let mut g = Guacamole::default();
        assert_eq!(60684u16, u16::from_guacamole(&mut (), &mut g));
        assert_eq!(20313u16, u16::from_guacamole(&mut (), &mut g));
        assert_eq!(6582u16, u16::from_guacamole(&mut (), &mut g));
        assert_eq!(58955u16, u16::from_guacamole(&mut (), &mut g));
    }

    #[test]
    fn i32_consts() {
        let mut g = Guacamole::default();
        assert_eq!(1331293452i32, i32::from_guacamole(&mut (), &mut g));
        assert_eq!(-431285834i32, i32::from_guacamole(&mut (), &mut g));
        assert_eq!(-242250468i32, i32::from_guacamole(&mut (), &mut g));
        assert_eq!(624269013i32, i32::from_guacamole(&mut (), &mut g));
    }

    #[test]
    fn u32_consts() {
        let mut g = Guacamole::default();
        assert_eq!(1331293452u32, u32::from_guacamole(&mut (), &mut g));
        assert_eq!(3863681462u32, u32::from_guacamole(&mut (), &mut g));
        assert_eq!(4052716828u32, u32::from_guacamole(&mut (), &mut g));
        assert_eq!(624269013u32, u32::from_guacamole(&mut (), &mut g));
    }

    #[test]
    fn i64_consts() {
        let mut g = Guacamole::default();
        assert_eq!(
            -1852358550926791412i64,
            i64::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(2681214998793915676i64, i64::from_guacamole(&mut (), &mut g));
        assert_eq!(4092481979873166344i64, i64::from_guacamole(&mut (), &mut g));
        assert_eq!(
            -5424627454596165171i64,
            i64::from_guacamole(&mut (), &mut g)
        );
    }

    #[test]
    fn u64_consts() {
        let mut g = Guacamole::default();
        assert_eq!(
            16594385522782760204u64,
            u64::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(2681214998793915676u64, u64::from_guacamole(&mut (), &mut g));
        assert_eq!(4092481979873166344u64, u64::from_guacamole(&mut (), &mut g));
        assert_eq!(
            13022116619113386445u64,
            u64::from_guacamole(&mut (), &mut g)
        );
    }

    #[test]
    fn i128_consts() {
        let mut g = Guacamole::default();
        assert_eq!(
            49459686889342826596546834789656292620i128,
            i128::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(
            -100066714350153939649187475127612799992i128,
            i128::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(
            -86802739999401378514630235182376832904i128,
            i128::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(
            116000783475269626238547120300001778485i128,
            i128::from_guacamole(&mut (), &mut g)
        );
    }

    #[test]
    fn u128_consts() {
        let mut g = Guacamole::default();
        assert_eq!(
            49459686889342826596546834789656292620u128,
            u128::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(
            240215652570784523814187132304155411464u128,
            u128::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(
            253479626921537084948744372249391378552u128,
            u128::from_guacamole(&mut (), &mut g)
        );
        assert_eq!(
            116000783475269626238547120300001778485u128,
            u128::from_guacamole(&mut (), &mut g)
        );
    }

    #[test]
    fn f32_consts() {
        let mut g = Guacamole::default();
        fn approx(lhs: f32, rhs: f32) -> bool {
            lhs + f32::EPSILON > rhs && rhs + f32::EPSILON > lhs
        }
        assert!(approx(0.3512733, f32::from_guacamole(&mut (), &mut g)));
        assert!(approx(0.10043806, f32::from_guacamole(&mut (), &mut g)));
        assert!(approx(0.11288899, f32::from_guacamole(&mut (), &mut g)));
        assert!(approx(0.94359666, f32::from_guacamole(&mut (), &mut g)));
    }

    #[test]
    fn f64_consts() {
        let mut g = Guacamole::default();
        fn approx(lhs: f64, rhs: f64) -> bool {
            lhs + f64::EPSILON > rhs && rhs + f64::EPSILON > lhs
        }
        assert!(approx(
            0.34688868997855726,
            f64::from_guacamole(&mut (), &mut g)
        ));
        assert!(approx(
            0.7136161617026147,
            f64::from_guacamole(&mut (), &mut g)
        ));
        assert!(approx(
            0.42236662661995317,
            f64::from_guacamole(&mut (), &mut g)
        ));
        assert!(approx(
            0.31137933809655505,
            f64::from_guacamole(&mut (), &mut g)
        ));
    }

    #[test]
    fn tuple_consts() {
        let mut g = Guacamole::default();
        let x: (u8, u8, u8, u8) = FromGuacamole::from_guacamole(&mut (), &mut g);
        assert_eq!((12u8, 237u8, 89u8, 79u8), x);
    }

    #[test]
    fn array_consts() {
        let mut g = Guacamole::default();
        let x: [u8; 4] = FromGuacamole::from_guacamole(&mut (), &mut g);
        assert_eq!([12u8, 237u8, 89u8, 79u8], x);
    }

    #[test]
    fn char_util() {
        for i in 0..(1 << 24) {
            char_from_u24(i);
        }
    }
}
