use super::super::bit_array::BitArray;

//////////////////////////////////////////////// u63 ///////////////////////////////////////////////

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct u63 {
    x: u64,
}

impl u63 {
    fn must(x: u64) -> Self {
        match Self::new(x) {
            Some(x) => x,
            None => {
                panic!("`must` called with invalid word");
            },
        }
    }

    fn new(x: u64) -> Option<Self> {
        if x & (1u64 << 63) != 0 {
            return None
        }
        Some(Self {
            x,
        })
    }
}

//////////////////////////////////////// SixtyThreeBitWords ////////////////////////////////////////

pub struct SixtyThreeBitWords<'a> {
    bytes: BitArray<'a>,
    index: usize,
}

impl<'a> SixtyThreeBitWords<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            bytes: BitArray::new(buf),
            index: 0,
        }
    }
}

impl<'a> Iterator for SixtyThreeBitWords<'a> {
    type Item = u63;

    fn next(&mut self) -> Option<u63> {
        let bits = self.bytes.bits();
        if bits > self.index {
            let amt = if self.index + 63 > bits { bits - self.index } else { 63 };
            let answer = u63::must(self.bytes.load(self.index, amt));
            self.index += amt;
            Some(answer)
        } else {
            None
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sixty_three_new_works() {
        let x = u63::new((1u64 << 63) - 1);
        assert!(x.is_some());
    }

    #[test]
    fn sixty_four_fails() {
        let x = u63::new(1u64 << 63);
        assert!(x.is_none());
    }

    #[test]
    fn sixty_three_must() {
        let x = u63::must((1u64 << 63) - 1);
        assert_eq!((1u64 << 63) - 1, x.x);
    }

    #[test]
    fn sixty_three_bit_words() {
        // s = ''
        // for i in range(128):
        //     s += ''.join(reversed(bin(i)[2:].rjust(63, '0')))
        // 
        // while s:
        //     byte = int(''.join(reversed(s[0:8])), 2)
        //     print('{},'.format(hex(byte)))
        //     s = s[8:]
        let bytes: &[u8] = &[
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x60,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x40,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x28,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x18,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xe,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x8,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x4,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x2,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x60, 0x1,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xc0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x68, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x38, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1e, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x10, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x8, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x4, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x60, 0x2, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x40, 0x1, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0xa8, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x58, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x2e, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x18, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x80, 0xc, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x80, 0x6, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x60, 0x3, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0xc0, 0x1, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0xe8, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x78, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x3e, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x20, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x80, 0x10, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x80, 0x8, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x60, 0x4, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x40, 0x2, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x28, 0x1, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x98, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x4e, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x28, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x80, 0x14, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x80, 0xa, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x60, 0x5, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0xc0, 0x2, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x68, 0x1, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0xb8, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x5e, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x30, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x80, 0x18, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x80, 0xc, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x60, 0x6, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x40, 0x3, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0xa8, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0xd8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x6e, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x38, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x80, 0x1c, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x80, 0xe, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x60, 0x7, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0xc0, 0x3, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0xe8, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0xf8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x7e, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x40, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80,
            0x20, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80,
            0x10, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x60,
            0x8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x40,
            0x4, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x28,
            0x2, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x18,
            0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x8e,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x48,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x24,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x12,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x60, 0x9,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xc0, 0x4,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x68, 0x2,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x38, 0x1,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x9e, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x50, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x28, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x80, 0x14, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x60, 0xa, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x40, 0x5, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0xa8, 0x2, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x58, 0x1, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0xae, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x0, 0x58, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x80, 0x2c, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x80, 0x16, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x60, 0xb, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0xc0, 0x5, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0xe8, 0x2, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x78, 0x1, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0xbe, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x60, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x80, 0x30, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x80, 0x18, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x60, 0xc, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x40, 0x6, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x28, 0x3, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x98, 0x1, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0xce, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x68, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x80, 0x34, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x80, 0x1a, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x60, 0xd, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0xc0, 0x6, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x68, 0x3, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0xb8, 0x1, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0xde, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x70, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x80, 0x38, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x80, 0x1c, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x60, 0xe, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x40, 0x7, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0xa8, 0x3, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0xd8, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0xee, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x78, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x80, 0x3c, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x80, 0x1e, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x60, 0xf, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0xc0, 0x7, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0xe8, 0x3, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0xf8, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0xfe, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        for (exp, word) in SixtyThreeBitWords::new(bytes).enumerate() {
            assert_eq!(u63::must(exp as u64), word, "exp={}", exp);
        }
    }
}
