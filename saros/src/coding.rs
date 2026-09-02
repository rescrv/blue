///////////////////////////////////////////// constants ////////////////////////////////////////////

pub const MAX_BITS_FOR_GAMMA: usize = 128;
pub const MAX_BITS_FOR_DELTA: usize = 77;

////////////////////////////////////////// gamma encoding //////////////////////////////////////////

pub fn gamma(x: u64) -> (u128, usize) {
    assert!(x < u64::MAX);
    let x: u64 = x + 1;
    let zeros: u32 = x.leading_zeros();
    let width: u32 = 64 - zeros;
    assert!(width > 0);
    let x: u64 = x.reverse_bits();
    let mut x: u128 = x.into();
    x >>= zeros;
    x <<= width;
    (x, width as usize * 2)
}

pub fn ungamma(x: u128) -> (u64, usize) {
    let width = x.trailing_zeros();
    let x: u64 = (x >> width) as u64;
    let x: u64 = x.reverse_bits();
    ((x >> (64 - width)) - 1, 2 * width as usize)
}

pub fn is_gamma(x: u128, x_sz: usize) -> bool {
    let width = x.trailing_zeros();
    x_sz >= width as usize * 2 && width > 0
}

////////////////////////////////////////// delta encoding //////////////////////////////////////////

pub fn delta(x: u64) -> (u128, usize) {
    let zeros: u32 = x.leading_zeros();
    let width = 64 - zeros;
    let (mut gamma, gamma_width) = gamma(width.into());
    if width == 0 {
        (gamma, gamma_width)
    } else {
        let width: usize = width as usize - 1;
        let x = x & !(1 << width);
        let x: u128 = x.into();
        gamma |= x << gamma_width;
        (gamma, gamma_width + width)
    }
}

pub fn undelta(x: u128) -> (u64, usize) {
    let (width, consumed) = ungamma(x);
    if width == 0 {
        (0, consumed)
    } else {
        let x: u64 = (x >> consumed) as u64;
        let x = if width > 0 && width < 64 {
            x & ((1 << width) - 1)
        } else {
            x
        };
        (x | (1 << (width - 1)), width as usize + consumed - 1)
    }
}

pub fn is_delta(x: u128, x_sz: usize) -> bool {
    if is_gamma(x, x_sz) {
        let (width, consumed) = ungamma(x);
        if width == 0 {
            consumed <= x_sz
        } else {
            width + consumed as u64 - 1 <= x_sz as u64
        }
    } else {
        false
    }
}

//////////////////////////////////////////// bit stream ////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    byte: u8,
    bits: usize,
}

impl BitWriter {
    pub fn push_bit(&mut self, bit: bool) {
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

    pub fn push_bits(&mut self, mut word: u64, mut bits: usize) {
        assert!(bits <= 64);
        while bits > 0 {
            let available = 8 - self.bits;
            let take = std::cmp::min(available, bits);
            let mask = if take == 64 {
                u64::MAX
            } else {
                (1u64 << take) - 1
            };
            self.byte |= ((word & mask) as u8) << self.bits;
            self.bits += take;
            word >>= take;
            bits -= take;
            if self.bits == 8 {
                self.bytes.push(self.byte);
                self.byte = 0;
                self.bits = 0;
            }
        }
    }

    pub fn seal(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.bytes.push(self.byte);
        }
        self.bytes
    }
}

#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub fn read_bit(&mut self) -> Option<bool> {
        let byte = *self.bytes.get(self.offset >> 3)?;
        let bit = byte & (1 << (self.offset & 7)) != 0;
        self.offset += 1;
        Some(bit)
    }

    pub fn read_bits(&mut self, mut bits: usize) -> Option<u64> {
        assert!(bits <= 64);
        let mut word = 0u64;
        let mut shift = 0usize;
        while bits > 0 {
            let byte = *self.bytes.get(self.offset >> 3)?;
            let bit_offset = self.offset & 7;
            let available = 8 - bit_offset;
            let take = std::cmp::min(available, bits);
            let mask = (1u64 << take) - 1;
            let piece = ((byte >> bit_offset) as u64) & mask;
            word |= piece << shift;
            self.offset += take;
            shift += take;
            bits -= take;
        }
        Some(word)
    }
}

//////////////////////////////////////////// gorilla f64 ///////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct GorillaEncoder {
    writer: BitWriter,
    prev: u64,
    leading: usize,
    trailing: usize,
    have_window: bool,
}

impl GorillaEncoder {
    pub fn new(first: u64) -> Self {
        Self {
            writer: BitWriter::default(),
            prev: first,
            leading: 0,
            trailing: 0,
            have_window: false,
        }
    }

    pub fn push(&mut self, value: u64) {
        let xor = self.prev ^ value;
        if xor == 0 {
            self.writer.push_bit(false);
            self.prev = value;
            return;
        }
        let mut leading = xor.leading_zeros() as usize;
        let trailing = xor.trailing_zeros() as usize;
        if self.have_window && self.leading <= leading && self.trailing <= trailing {
            self.writer.push_bits(0b01, 2);
            let width = 64 - self.leading - self.trailing;
            self.writer.push_bits(xor >> self.trailing, width);
        } else {
            leading = std::cmp::min(leading, 31);
            let width = 64 - leading - trailing;
            self.writer.push_bits(0b11, 2);
            self.writer.push_bits(leading as u64, 5);
            self.writer
                .push_bits(if width == 64 { 0 } else { width as u64 }, 6);
            self.writer.push_bits(xor >> trailing, width);
            self.leading = leading;
            self.trailing = trailing;
            self.have_window = true;
        }
        self.prev = value;
    }

    pub fn seal(self) -> Vec<u8> {
        self.writer.seal()
    }
}

#[derive(Clone, Debug)]
pub struct GorillaDecoder<'a> {
    reader: BitReader<'a>,
    prev: u64,
    leading: usize,
    trailing: usize,
    have_window: bool,
}

impl<'a> GorillaDecoder<'a> {
    pub fn new(first: u64, bytes: &'a [u8]) -> Self {
        Self {
            reader: BitReader::new(bytes),
            prev: first,
            leading: 0,
            trailing: 0,
            have_window: false,
        }
    }
}

impl Iterator for GorillaDecoder<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.reader.read_bit()? {
            return Some(self.prev);
        }
        let reuse_window = !self.reader.read_bit()?;
        let (leading, trailing, width) = if reuse_window {
            if !self.have_window {
                return None;
            }
            let width = 64 - self.leading - self.trailing;
            (self.leading, self.trailing, width)
        } else {
            let leading = self.reader.read_bits(5)? as usize;
            let width = match self.reader.read_bits(6)? as usize {
                0 => 64,
                width => width,
            };
            if leading + width > 64 {
                return None;
            }
            let trailing = 64 - leading - width;
            self.leading = leading;
            self.trailing = trailing;
            self.have_window = true;
            (leading, trailing, width)
        };
        debug_assert!(leading + trailing + width == 64);
        let meaningful = self.reader.read_bits(width)?;
        let value = self.prev ^ (meaningful << trailing);
        self.prev = value;
        Some(value)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_ungamma() {
        fn round_trip(x: u64, exp_g: u128, exp_w: usize) {
            let (ret_g, ret_w) = gamma(x);
            assert_eq!(exp_g, ret_g);
            assert_eq!(exp_w, ret_w);
            let (ret_x, ret_w) = ungamma(exp_g);
            assert_eq!(x, ret_x);
            assert_eq!(exp_w, ret_w);
            for w in 0..exp_w {
                assert!(!is_gamma(exp_g, w));
            }
            for w in exp_w..128 {
                assert!(is_gamma(exp_g, w));
            }
        }
        round_trip(0, 2, 2);
        round_trip(1, 4, 4);
        round_trip(2, 12, 4);
        round_trip(3, 8, 6);
        round_trip(4, 40, 6);
        round_trip(5, 24, 6);
        round_trip(7, 16, 8);
        round_trip(8, 144, 8);
        round_trip(9, 80, 8);
        round_trip(15, 32, 10);
        round_trip(16, 544, 10);
        round_trip(17, 288, 10);
        round_trip(31, 64, 12);
        round_trip(32, 2112, 12);
        round_trip(33, 1088, 12);
        round_trip(63, 128, 14);
        round_trip(64, 8320, 14);
        round_trip(65, 4224, 14);
        round_trip(127, 256, 16);
        round_trip(128, 33024, 16);
        round_trip(129, 16640, 16);
        round_trip(255, 512, 18);
        round_trip(256, 131584, 18);
        round_trip(257, 66048, 18);

        round_trip(65535, 131072, 34);
        round_trip(65536, 8590065664, 34);
        round_trip(65537, 4295098368, 34);

        round_trip(16777215, 33554432, 50);
        round_trip(16777216, 562949986975744, 50);
        round_trip(16777217, 281475010265088, 50);

        round_trip(4294967295, 8589934592, 66);
        round_trip(4294967296, 36893488156009037824, 66);
        round_trip(4294967297, 18446744082299486208, 66);

        round_trip(1099511627775, 2199023255552, 82);
        round_trip(1099511627776, 2417851639231457372667904, 82);
        round_trip(1099511627777, 1208925819616828197961728, 82);

        round_trip(281474976710655, 562949953421312, 98);
        round_trip(281474976710656, 158456325028529238137041321984, 98);
        round_trip(281474976710657, 79228162514264900543497371648, 98);

        round_trip(72057594037927935, 144115188075855872, 114);
        round_trip(72057594037927936, 10384593717069655401176180734296064, 114);
        round_trip(72057594037927937, 5192296858534827772645684405075968, 114);

        round_trip(
            18446744073709551614,
            340282366920938463444927863358058659840,
            128,
        );
    }

    #[test]
    #[should_panic]
    fn gamma_limit() {
        gamma(18446744073709551615);
    }

    #[test]
    fn delta_undelta() {
        fn round_trip(x: u64, exp_d: u128, exp_w: usize) {
            let (ret_d, ret_w) = delta(x);
            assert_eq!(exp_d, ret_d);
            assert_eq!(exp_w, ret_w);
            let (ret_x, ret_w) = undelta(ret_d);
            assert_eq!(x, ret_x);
            assert_eq!(exp_w, ret_w);
            for w in 0..exp_w {
                assert!(!is_delta(exp_d, w));
            }
            for w in exp_w..128 {
                assert!(is_delta(exp_d, w));
            }
        }
        round_trip(1, 4, 4);
        round_trip(2, 12, 5);
        round_trip(3, 28, 5);
        round_trip(4, 8, 8);
        round_trip(5, 72, 8);
        round_trip(7, 200, 8);
        round_trip(8, 40, 9);
        round_trip(9, 104, 9);
        round_trip(15, 488, 9);
        round_trip(16, 24, 10);
        round_trip(17, 88, 10);
        round_trip(31, 984, 10);
        round_trip(32, 56, 11);
        round_trip(33, 120, 11);

        round_trip(63, 2040, 11);
        round_trip(64, 16, 14);
        round_trip(65, 272, 14);

        round_trip(127, 16144, 14);
        round_trip(128, 144, 15);
        round_trip(129, 400, 15);

        round_trip(255, 32656, 15);
        round_trip(256, 80, 16);
        round_trip(257, 336, 16);

        round_trip(65535, 33553952, 25);
        round_trip(65536, 288, 26);
        round_trip(65537, 1312, 26);

        round_trip(16777215, 8589934176, 33);
        round_trip(16777216, 352, 34);
        round_trip(16777217, 1376, 34);

        round_trip(4294967295, 8796093020224, 43);
        round_trip(4294967296, 1088, 44);
        round_trip(4294967297, 5184, 44);

        round_trip(1099511627775, 2251799813683520, 51);
        round_trip(1099511627776, 1344, 52);
        round_trip(1099511627777, 5440, 52);

        round_trip(281474976710655, 576460752303421632, 59);
        round_trip(281474976710656, 1216, 60);
        round_trip(281474976710657, 5312, 60);

        round_trip(72057594037927935, 147573952589676411328, 67);
        round_trip(72057594037927936, 1472, 68);
        round_trip(72057594037927937, 5568, 68);

        round_trip(18446744073709551614, 151115727451828646813824, 77);
        round_trip(18446744073709551615, 151115727451828646830208, 77);

        // Cases that tickled bugs.
        round_trip(18014398509481984, 448, 66);
        round_trip(7184726270831141472, 42156691495383098458240, 76);
    }

    #[test]
    fn delta_undelta_overrun_bug() {
        // This bug happens when a valid prefix code has subsequent bits.
        const TICKLER: u64 = 18014398509481984;
        let (encoded, width) = delta(TICKLER);
        assert_eq!(448, encoded);
        assert_eq!(66, width);
        let (decoded, width) = undelta(encoded);
        assert_eq!(TICKLER, decoded);
        assert_eq!(66, width);
        let (decoded, width) = undelta(147573952589676413376);
        assert_eq!(TICKLER, decoded);
        assert_eq!(66, width);
    }

    #[test]
    fn bit_stream_round_trips_words() {
        let mut writer = BitWriter::default();
        writer.push_bit(true);
        writer.push_bits(0b101, 3);
        writer.push_bits(u64::MAX, 64);
        writer.push_bits(0x1234, 16);
        let bytes = writer.seal();
        let mut reader = BitReader::new(&bytes);
        assert_eq!(Some(true), reader.read_bit());
        assert_eq!(Some(0b101), reader.read_bits(3));
        assert_eq!(Some(u64::MAX), reader.read_bits(64));
        assert_eq!(Some(0x1234), reader.read_bits(16));
    }

    #[test]
    fn gorilla_round_trips_f64_bits() {
        let values = [
            0.0f64.to_bits(),
            0.0f64.to_bits(),
            (-0.0f64).to_bits(),
            1.5f64.to_bits(),
            f64::INFINITY.to_bits(),
            f64::NEG_INFINITY.to_bits(),
            0x7ff8_1234_5678_9abcu64,
            0x7ff8_1234_5678_9abcu64,
            42.0f64.to_bits(),
        ];
        let mut encoder = GorillaEncoder::new(values[0]);
        for value in values.iter().copied().skip(1) {
            encoder.push(value);
        }
        let bytes = encoder.seal();
        let mut decoder = GorillaDecoder::new(values[0], &bytes);
        let mut decoded = vec![values[0]];
        for _ in 1..values.len() {
            decoded.push(decoder.next().unwrap());
        }
        assert_eq!(values.as_slice(), decoded.as_slice());
    }

    ////////////////////////////////////////////// proptests //////////////////////////////////////////////

    /// Round-trips a gorilla series through the encoder and decoder.
    ///
    /// The first sample seeds both `GorillaEncoder` and `GorillaDecoder` and is
    /// never written into the bit stream; the decoder reproduces only the
    /// remainder, so we re-prepend the seed before comparing to the original.
    fn gorilla_round_trip(values: &[u64]) -> Result<(), proptest::test_runner::TestCaseError> {
        let first = values[0];
        let mut encoder = GorillaEncoder::new(first);
        for &value in values.iter().skip(1) {
            encoder.push(value);
        }
        let bytes = encoder.seal();
        let mut decoder = GorillaDecoder::new(first, &bytes);
        let mut decoded = vec![first];
        // The gorilla stream carries no end-marker: seal() zero-pads the final
        // byte, so the decoder must be driven by the known sample count
        // (mirroring how the store pairs values with timestamps) rather than by
        // iterator exhaustion, which would read padding bits as repeated values.
        for _ in 1..values.len() {
            let value = decoder
                .next()
                .expect("decoder exhausted before reproducing all samples");
            decoded.push(value);
        }
        proptest::prop_assert_eq!(values, decoded.as_slice());
        Ok(())
    }

    proptest::prop_compose! {
        pub fn arb_gamma_value()(x in 0u64..u64::MAX) -> u64 {
            x
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_gamma_round_trip(x in arb_gamma_value()) {
            // gamma() panics on u64::MAX, so the strategy spans [0, u64::MAX);
            // every value must round-trip and satisfy the is_gamma prefix test.
            let (g, w) = gamma(x);
            let (x2, w2) = ungamma(g);
            proptest::prop_assert_eq!(x, x2);
            proptest::prop_assert_eq!(w, w2);
            for sz in 0..w {
                proptest::prop_assert!(
                    !is_gamma(g, sz),
                    "is_gamma({:#x}, {}) should be false (w={})",
                    g, sz, w
                );
            }
            for sz in w..=128 {
                proptest::prop_assert!(
                    is_gamma(g, sz),
                    "is_gamma({:#x}, {}) should be true (w={})",
                    g, sz, w
                );
            }
        }
    }

    proptest::prop_compose! {
        pub fn arb_delta_value()(x in proptest::arbitrary::any::<u64>()) -> u64 {
            x
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_delta_round_trip(x in arb_delta_value()) {
            // delta() accepts the whole u64 domain, u64::MAX included.
            let (d, w) = delta(x);
            let (x2, w2) = undelta(d);
            proptest::prop_assert_eq!(x, x2);
            proptest::prop_assert_eq!(w, w2);
            for sz in 0..w {
                proptest::prop_assert!(
                    !is_delta(d, sz),
                    "is_delta({:#x}, {}) should be false (w={})",
                    d, sz, w
                );
            }
            for sz in w..=128 {
                proptest::prop_assert!(
                    is_delta(d, sz),
                    "is_delta({:#x}, {}) should be true (w={})",
                    d, sz, w
                );
            }
        }
    }

    proptest::prop_compose! {
        pub fn arb_bits()(bits in proptest::collection::vec(
            proptest::arbitrary::any::<bool>(), 0..1024)) -> Vec<bool> {
            bits
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_bit_stream_bits(bits in arb_bits()) {
            let mut writer = BitWriter::default();
            for &bit in bits.iter() {
                writer.push_bit(bit);
            }
            let bytes = writer.seal();
            let mut reader = BitReader::new(&bytes);
            for (idx, &expected) in bits.iter().enumerate() {
                let got = reader.read_bit().expect("reader ran out before writer");
                proptest::prop_assert_eq!(expected, got, "idx={}", idx);
            }
            // seal() zero-pads the final byte, so bits past the pushed run are
            // readable padding; the round-trip property is that the first
            // `bits` reads reproduce the pushed stream, which the loop above
            // checks.  (The gorilla decoder relies on this same count-driven
            // discipline rather than an end-marker.)
        }
    }

    proptest::prop_compose! {
        pub fn arb_words()(words in proptest::collection::vec(
            (proptest::arbitrary::any::<u64>(), 1usize..=64usize), 0..256)) -> Vec<(u64, usize)> {
            words
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_bit_stream_words(words in arb_words()) {
            let mut writer = BitWriter::default();
            for &(word, bits) in words.iter() {
                writer.push_bits(word, bits);
            }
            let bytes = writer.seal();
            let mut reader = BitReader::new(&bytes);
            for (idx, &(word, bits)) in words.iter().enumerate() {
                let got = reader.read_bits(bits).expect("reader ran out before writer");
                // push_bits emits only the low `bits` bits of `word`.
                let expected = if bits == 64 {
                    word
                } else {
                    word & ((1u64 << bits) - 1)
                };
                proptest::prop_assert_eq!(
                    expected, got,
                    "idx={} bits={} word={:#x}",
                    idx, bits, word
                );
            }
        }
    }

    proptest::prop_compose! {
        pub fn arb_bit_ops()(ops in proptest::collection::vec(
            (proptest::arbitrary::any::<u8>(),
             proptest::arbitrary::any::<u64>(),
             1usize..=64usize),
            0..256)) -> Vec<(u8, u64, usize)> {
            ops
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_bit_stream_mixed(ops in arb_bit_ops()) {
            // Each triple is (kind, word, bits).  An even `kind` emits a single
            // bit (the low bit of `word`); an odd `kind` emits `bits` bits of
            // `word`.  This interleaves push_bit and push_bits across byte
            // boundaries, which the gorilla codec relies on.
            let mut writer = BitWriter::default();
            for &(kind, word, bits) in ops.iter() {
                if kind % 2 == 0 {
                    writer.push_bit(word & 1 != 0);
                } else {
                    writer.push_bits(word, bits);
                }
            }
            let bytes = writer.seal();
            let mut reader = BitReader::new(&bytes);
            for (idx, &(kind, word, bits)) in ops.iter().enumerate() {
                if kind % 2 == 0 {
                    let got = reader.read_bit().expect("reader ran out before writer");
                    proptest::prop_assert_eq!(word & 1 != 0, got, "idx={}", idx);
                } else {
                    let got = reader.read_bits(bits).expect("reader ran out before writer");
                    let expected = if bits == 64 {
                        word
                    } else {
                        word & ((1u64 << bits) - 1)
                    };
                    proptest::prop_assert_eq!(
                        expected, got,
                        "idx={} bits={} word={:#x}",
                        idx, bits, word
                    );
                }
            }
        }
    }

    proptest::prop_compose! {
        pub fn arb_gorilla_series()(values in proptest::collection::vec(
            proptest::arbitrary::any::<u64>(), 1..512)) -> Vec<u64> {
            values
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_gorilla_round_trip(values in arb_gorilla_series()) {
            gorilla_round_trip(&values)?;
        }
    }

    proptest::prop_compose! {
        pub fn arb_gorilla_clustered_series()(
            base in proptest::arbitrary::any::<u64>(),
            perturbations in proptest::collection::vec(
                proptest::arbitrary::any::<u64>(), 0..512),
        ) -> Vec<u64> {
            // Perturb only the low 16 bits so consecutive XORs share a large
            // leading-zero run; this repeatedly drives the decoder through the
            // "reuse the previous window" path, not just the "open a new window"
            // path that uniformly-random data mostly hits.
            let mut values = vec![base];
            let mut cur = base;
            for &p in perturbations.iter() {
                cur ^= p & 0xffff;
                values.push(cur);
            }
            values
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_gorilla_clustered_series(values in arb_gorilla_clustered_series()) {
            gorilla_round_trip(&values)?;
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_gorilla_constant_series(
            value in proptest::arbitrary::any::<u64>(),
            count in 1usize..512,
        ) {
            // A run of identical samples exercises the xor == 0 fast path, which
            // emits a single zero control bit per sample.
            let values = vec![value; count];
            gorilla_round_trip(&values)?;
        }
    }
}
