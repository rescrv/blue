////////////////////////////////////////////// Encoder /////////////////////////////////////////////

/// Encoder captures prefix-free codes from a text document.  It is necessary that codes be
/// prefix-free starting with the least significant bit.
pub trait Encoder {
    /// Construct a new encoder, returning Err if the text is not amenable to encoding (usually
    /// because there are too many symbols in the text).
    fn construct(text: &[u32]) -> Self;

    /// Encode symbol `t`, returning None if t is not a symbol of the text.
    fn encode(&self, t: u32) -> Option<(u32, u8)>;
    /// Decode the symbol `e` that is of size `s` bits.
    fn decode(&self, e: u32, s: u8) -> Option<u32>;
}

///////////////////////////////////////// FixedWidthEncoder ////////////////////////////////////////

/// FixedWidthEncoder maps the text to characters on [0, log(n)).
#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FixedWidthEncoder {
    #[prototk(1, uint32)]
    chars: Vec<u32>,
}

impl Encoder for FixedWidthEncoder {
    fn construct(text: &[u32]) -> Self {
        let mut chars = text.to_vec();
        chars.sort();
        chars.dedup();
        chars.shrink_to_fit();
        // SAFETY(rescrv): there are at most u32::MAX unique symbols in the input.
        assert!(chars.len() <= u32::MAX as usize);
        Self { chars }
    }

    fn encode(&self, t: u32) -> Option<(u32, u8)> {
        let position: u32 = self.chars.binary_search(&t).ok()?.try_into().ok()?;
        let bits = std::cmp::max(self.chars.len(), 2)
            .next_power_of_two()
            .ilog2()
            .try_into()
            .ok()?;
        Some((position, bits))
    }

    fn decode(&self, v: u32, _: u8) -> Option<u32> {
        let v: usize = v.try_into().ok()?;
        self.chars.get(v).copied()
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::{Encoder, FixedWidthEncoder};

    #[test]
    fn fixed_width_empty() {
        let chars = vec![];
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!(None, encoder.encode(0u32));
    }

    #[test]
    fn fixed_width_0() {
        let chars = vec![0u32];
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!((0, 1), encoder.encode(0u32).unwrap());
        assert_eq!(None, encoder.encode(1u32));
    }

    #[test]
    fn fixed_width_0_1() {
        let chars = vec![0u32, 1u32];
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!((0, 1), encoder.encode(0u32).unwrap());
        assert_eq!((1, 1), encoder.encode(1u32).unwrap());
    }

    #[test]
    fn fixed_width_chars() {
        let chars: Vec<u32> = "AaBbCcDdEeFfNnBananaMississippi"
            .chars()
            .map(|c| c as u32)
            .collect();
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!((0, 5), encoder.encode('A' as u32).unwrap());
        assert_eq!((1, 5), encoder.encode('B' as u32).unwrap());
        assert_eq!((2, 5), encoder.encode('C' as u32).unwrap());
        assert_eq!((8, 5), encoder.encode('a' as u32).unwrap());
        assert_eq!((9, 5), encoder.encode('b' as u32).unwrap());
        assert_eq!((10, 5), encoder.encode('c' as u32).unwrap());
        for c in chars.iter().copied() {
            let (v, s) = encoder.encode(c).unwrap();
            assert_eq!(Some(c), encoder.decode(v, s));
        }
        assert_eq!(None, encoder.encode('q' as u32));
        assert_eq!(None, encoder.encode('z' as u32));
    }
}
