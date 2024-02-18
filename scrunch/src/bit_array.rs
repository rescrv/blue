///////////////////////////////////////////// BitArray /////////////////////////////////////////////

/// A [BitArray] is a sequence of bits, from which variable-size words can be drawn from adjacent
/// bits.  It is not a BitVector, but the underlying structure under a bit vector.
#[derive(Clone, Debug)]
pub struct BitArray<'a> {
    bytes: &'a [u8],
}

impl<'a> BitArray<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn load(&self, index: usize, mut bits: usize) -> Option<u64> {
        let mut byte_index = index >> 3;
        let mut bit_index = index & 7;
        assert!(bits <= 64);
        let mut x = 0u64;
        let mut xlen = 0usize;
        while bits > 0 {
            // We take the highest order bits from this byte.
            let mut byte: u64 = (self.bytes.get(byte_index)? >> bit_index).into();
            let bits_from_this_byte = std::cmp::min(8 - bit_index, bits);
            byte &= (1u64 << bits_from_this_byte) - 1;
            x |= byte << xlen;
            xlen += bits_from_this_byte;
            bits -= bits_from_this_byte;
            byte_index += 1;
            bit_index = 0;
        }
        Some(x)
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        let byte_index = index >> 3;
        let bit_index = index & 7;
        Some(self.bytes.get(byte_index)? & (1 << bit_index) != 0)
    }

    pub fn bits(&self) -> usize {
        self.bytes.len() << 3
    }
}

impl<'a> AsRef<[u8]> for BitArray<'a> {
    fn as_ref(&self) -> &[u8] {
        self.bytes
    }
}

////////////////////////////////////////////// Builder /////////////////////////////////////////////

// TODO(rescrv):  Make builder tie into the builder and not return a vec.

/// A Builder creates a BitVector one bit at a time.
#[derive(Debug, Default)]
pub struct Builder {
    bytes: Vec<u8>,
    byte: u8,
    bits: usize,
}

impl Builder {
    pub fn with_capacity(bits: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(1 + (bits >> 3)),
            byte: 0u8,
            bits: 0usize,
        }
    }

    pub fn push(&mut self, bit: bool) {
        if bit {
            self.byte |= 1 << self.bits;
        }
        self.bits += 1;
        if self.bits == 8 {
            self.bytes.push(self.byte);
            self.byte = 0;
            self.bits = 0;
        }
    }

    pub fn push_word(&mut self, word: u64, bits: usize) {
        assert!(bits < 64);
        assert!(word & !((1u64 << bits) - 1) == 0);
        for i in 0..bits {
            self.push(word & (1u64 << i) != 0);
        }
    }

    pub fn seal(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.bytes.push(self.byte);
        }
        self.bytes
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_zeroes() {
        let buf: [u8; 8] = [0xee, 0xff, 0xc0, 0x00, 0x0d, 0xf0, 0xaf, 0x1e];
        let ba = BitArray::new(&buf);

        // Test loading zero at the byte boundaries.
        assert_eq!(Some(0x00u64), ba.load(0, 0));
        assert_eq!(Some(0x00u64), ba.load(1, 0));
        assert_eq!(Some(0x00u64), ba.load(2, 0));
        assert_eq!(Some(0x00u64), ba.load(3, 0));
        assert_eq!(Some(0x00u64), ba.load(4, 0));
        assert_eq!(Some(0x00u64), ba.load(5, 0));
        assert_eq!(Some(0x00u64), ba.load(6, 0));
        assert_eq!(Some(0x00u64), ba.load(7, 0));
    }

    #[test]
    fn load_up_to_64_bits() {
        let buf: [u8; 8] = [0xee, 0xff, 0xc0, 0x00, 0x0d, 0xf0, 0xaf, 0x1e];
        let ba = BitArray::new(&buf);

        // Test cases generated from Python:
        assert_eq!(Some(0x00u64), ba.load(0, 1));
        assert_eq!(Some(0x02u64), ba.load(0, 2));
        assert_eq!(Some(0x06u64), ba.load(0, 3));
        assert_eq!(Some(0x0eu64), ba.load(0, 4));
        assert_eq!(Some(0x0eu64), ba.load(0, 5));
        assert_eq!(Some(0x2eu64), ba.load(0, 6));
        assert_eq!(Some(0x6eu64), ba.load(0, 7));
        assert_eq!(Some(0xeeu64), ba.load(0, 8));

        assert_eq!(Some(0x01eeu64), ba.load(0, 9));
        assert_eq!(Some(0x03eeu64), ba.load(0, 10));
        assert_eq!(Some(0x07eeu64), ba.load(0, 11));
        assert_eq!(Some(0x0feeu64), ba.load(0, 12));
        assert_eq!(Some(0x1feeu64), ba.load(0, 13));
        assert_eq!(Some(0x3feeu64), ba.load(0, 14));
        assert_eq!(Some(0x7feeu64), ba.load(0, 15));
        assert_eq!(Some(0xffeeu64), ba.load(0, 16));

        assert_eq!(Some(0x00ffeeu64), ba.load(0, 16));
        assert_eq!(Some(0x00ffeeu64), ba.load(0, 17));
        assert_eq!(Some(0x00ffeeu64), ba.load(0, 18));
        assert_eq!(Some(0x00ffeeu64), ba.load(0, 19));
        assert_eq!(Some(0x00ffeeu64), ba.load(0, 20));
        assert_eq!(Some(0x00ffeeu64), ba.load(0, 21));
        assert_eq!(Some(0x00ffeeu64), ba.load(0, 22));
        assert_eq!(Some(0x40ffeeu64), ba.load(0, 23));
        assert_eq!(Some(0xc0ffeeu64), ba.load(0, 24));

        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 25));
        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 26));
        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 27));
        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 28));
        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 29));
        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 30));
        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 31));
        assert_eq!(Some(0x00c0ffeeu64), ba.load(0, 32));

        assert_eq!(Some(0x0100c0ffeeu64), ba.load(0, 33));
        assert_eq!(Some(0x0100c0ffeeu64), ba.load(0, 34));
        assert_eq!(Some(0x0500c0ffeeu64), ba.load(0, 35));
        assert_eq!(Some(0x0d00c0ffeeu64), ba.load(0, 36));
        assert_eq!(Some(0x0d00c0ffeeu64), ba.load(0, 37));
        assert_eq!(Some(0x0d00c0ffeeu64), ba.load(0, 38));
        assert_eq!(Some(0x0d00c0ffeeu64), ba.load(0, 39));
        assert_eq!(Some(0x0d00c0ffeeu64), ba.load(0, 40));

        assert_eq!(Some(0x000d00c0ffeeu64), ba.load(0, 41));
        assert_eq!(Some(0x000d00c0ffeeu64), ba.load(0, 42));
        assert_eq!(Some(0x000d00c0ffeeu64), ba.load(0, 43));
        assert_eq!(Some(0x000d00c0ffeeu64), ba.load(0, 44));
        assert_eq!(Some(0x100d00c0ffeeu64), ba.load(0, 45));
        assert_eq!(Some(0x300d00c0ffeeu64), ba.load(0, 46));
        assert_eq!(Some(0x700d00c0ffeeu64), ba.load(0, 47));
        assert_eq!(Some(0xf00d00c0ffeeu64), ba.load(0, 48));

        assert_eq!(Some(0x01f00d00c0ffeeu64), ba.load(0, 49));
        assert_eq!(Some(0x03f00d00c0ffeeu64), ba.load(0, 50));
        assert_eq!(Some(0x07f00d00c0ffeeu64), ba.load(0, 51));
        assert_eq!(Some(0x0ff00d00c0ffeeu64), ba.load(0, 52));
        assert_eq!(Some(0x0ff00d00c0ffeeu64), ba.load(0, 53));
        assert_eq!(Some(0x2ff00d00c0ffeeu64), ba.load(0, 54));
        assert_eq!(Some(0x2ff00d00c0ffeeu64), ba.load(0, 55));
        assert_eq!(Some(0xaff00d00c0ffeeu64), ba.load(0, 56));

        assert_eq!(Some(0x00aff00d00c0ffeeu64), ba.load(0, 57));
        assert_eq!(Some(0x02aff00d00c0ffeeu64), ba.load(0, 58));
        assert_eq!(Some(0x06aff00d00c0ffeeu64), ba.load(0, 59));
        assert_eq!(Some(0x0eaff00d00c0ffeeu64), ba.load(0, 60));
        assert_eq!(Some(0x1eaff00d00c0ffeeu64), ba.load(0, 61));
        assert_eq!(Some(0x1eaff00d00c0ffeeu64), ba.load(0, 62));
        assert_eq!(Some(0x1eaff00d00c0ffeeu64), ba.load(0, 63));
        assert_eq!(Some(0x1eaff00d00c0ffeeu64), ba.load(0, 64));
    }

    #[test]
    fn push_word() {
        let mut builder = Builder::with_capacity(64);
        builder.push_word(0xffee, 16);
        builder.push_word(0x00c0, 16);
        builder.push_word(0xf00d, 16);
        builder.push_word(0x1eaf, 16);
        let buf = builder.seal();
        let ba = BitArray::new(&buf);
        assert_eq!(Some(0x1eaff00d00c0ffeeu64), ba.load(0, 64));
    }

    proptest::prop_compose! {
        pub fn arb_bit_vector()(bv in proptest::collection::vec(proptest::arbitrary::any::<bool>(), 0..65536)) -> Vec<bool> {
            bv
        }
    }

    proptest::proptest! {
        #[test]
        fn properties(bv in arb_bit_vector()) {
            let mut builder = Builder::with_capacity(bv.len());
            for bit in bv.iter() {
                builder.push(*bit);
            }
            let buf = builder.seal();
            let ba = BitArray::new(&buf);
            for (i, b) in bv.iter().enumerate() {
                assert_eq!(Some(*b), ba.get(i))
            }
            assert_eq!(None, bv.get(bv.len()));
        }
    }
}
