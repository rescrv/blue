use crate::{coding, Error};

/////////////////////////////////////////// DeltaEncoder ///////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct DeltaEncoder {
    bits: usize,
    bytes: Vec<u8>,
}

impl DeltaEncoder {
    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn push(&mut self, q_word: u64) -> Result<(), Error> {
        let (mut encoded, mut encoded_sz) = coding::delta(q_word);
        if (self.bits + encoded_sz) >> 3 >= 1 << 16 {
            return Err(Error::internal("encoder overrun:  too many bytes"));
        }
        let bit_offset = self.bits & 7;
        let mut bytes_idx = 3 + (self.bits >> 3);
        self.bits += encoded_sz;
        let mut offset = std::cmp::max(0, bit_offset);
        while encoded_sz > 0 {
            let amt = std::cmp::min(8 - offset, encoded_sz);
            let mask = (1u64 << amt) - 1;
            if self.bytes.len() <= bytes_idx {
                self.bytes.push(0);
            }
            self.bytes[bytes_idx] |= (encoded as u8 & mask as u8) << offset;
            encoded >>= amt;
            encoded_sz -= amt;
            bytes_idx += 1;
            offset = 0;
        }
        self.bytes[0] = self.bits as u8 & 7;
        if self.bytes[0] == 0 {
            self.bytes[0] = 8;
        }
        self.bytes[1] = self.bytes.len() as u8;
        self.bytes[2] = (self.bytes.len() >> 8) as u8;
        Ok(())
    }
}

impl Default for DeltaEncoder {
    fn default() -> Self {
        Self {
            bits: 0,
            bytes: vec![0, 3, 0],
        }
    }
}

impl AsRef<[u8]> for DeltaEncoder {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

///////////////////////////////////////// DeltaSliceDecoder ////////////////////////////////////////

#[derive(Clone, Debug)]
struct DeltaSliceDecoder<'a> {
    bits: usize,
    bytes: &'a [u8],
    next_offset_to_load: usize,
    encoded: u128,
    encoded_sz: usize,
}

impl<'a> DeltaSliceDecoder<'a> {
    #[cfg(test)]
    fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        let (bits, bytes_len) = Self::bits_and_bytes(bytes)?;
        let bytes = &bytes[..bytes_len];
        Ok(Self::from_bits_and_bytes(bits, bytes))
    }

    fn from_bits_and_bytes(bits: usize, bytes: &'a [u8]) -> Self {
        let next_offset_to_load = 24;
        let encoded = 0;
        let encoded_sz = 0;
        Self {
            bits,
            bytes,
            next_offset_to_load,
            encoded,
            encoded_sz,
        }
    }

    fn bits_and_bytes(bytes: &[u8]) -> Result<(usize, usize), Error> {
        if bytes.len() < 3 {
            return Err(Error::internal(format!(
                "expected at least three bytes in buffer; had: {}",
                bytes.len()
            )));
        }
        let bytes_in_buffer = bytes[1] as usize | ((bytes[2] as usize) << 8);
        if bytes_in_buffer < 3 {
            return Err(Error::internal(format!(
                "expected at buffer length to be at least 3; had: {}",
                bytes_in_buffer
            )));
        }
        let bits_in_last_byte = if bytes[0] == 0 { 8 } else { bytes[0] as usize };
        let bits = (bytes_in_buffer - 1) * 8 + bits_in_last_byte;
        assert_eq!((bits + 7) / 8, bytes_in_buffer);
        if bytes.len() < bytes_in_buffer {
            return Err(Error::internal(format!(
                "buffer length was {}, but only had {} bytes",
                bytes_in_buffer,
                bytes.len()
            )));
        }
        Ok((bits, bytes_in_buffer))
    }
}

impl<'a> Iterator for DeltaSliceDecoder<'a> {
    type Item = Result<u64, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if coding::is_delta(self.encoded, self.encoded_sz) {
                let (decoded, size) = coding::undelta(self.encoded);
                self.encoded >>= size;
                self.encoded_sz -= size;
                return Some(Ok(decoded));
            } else if self.next_offset_to_load >= self.bits {
                return None;
            } else if self.encoded_sz > coding::MAX_BITS_FOR_GAMMA {
                self.next_offset_to_load = self.bits;
                return Some(Err(Error::coding(format!(
                    "encoded_sz={:?} exceeds max bits for gamma at offset={}",
                    self.encoded_sz, self.next_offset_to_load
                ))));
            } else {
                let byte = self.next_offset_to_load / 8;
                self.encoded |= (self.bytes[byte] as u128) << self.encoded_sz;
                let amt = std::cmp::min(self.bits - self.next_offset_to_load, 8);
                self.next_offset_to_load += amt;
                self.encoded_sz += amt;
            }
        }
    }
}

/////////////////////////////////////////// DeltaDecoder ///////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct DeltaDecoder<'a> {
    slice: Option<DeltaSliceDecoder<'a>>,
    remain: &'a [u8],
    resets: usize,
}

impl<'a> DeltaDecoder<'a> {
    pub fn new(remain: &'a [u8]) -> Self {
        let slice = None;
        let resets = 0;
        Self {
            slice,
            remain,
            resets,
        }
    }

    pub fn drain(&mut self) {
        self.slice = None;
        self.remain = &[];
    }

    pub fn resets(&self) -> usize {
        self.resets
    }
}

impl<'a> Iterator for DeltaDecoder<'a> {
    type Item = Result<u64, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.slice.is_none() && !self.remain.is_empty() {
                let (bits, bytes_len) = match DeltaSliceDecoder::bits_and_bytes(self.remain) {
                    Ok(bits_and_bytes_len) => bits_and_bytes_len,
                    Err(err) => return Some(Err(err)),
                };
                assert!(bytes_len <= self.remain.len());
                let bytes = &self.remain[..bytes_len];
                self.remain = &self.remain[bytes_len..];
                self.slice = Some(DeltaSliceDecoder::from_bits_and_bytes(bits, bytes));
                self.resets += 1;
            } else if let Some(slice) = self.slice.as_mut() {
                if let Some(decoded) = slice.next() {
                    return Some(decoded);
                } else {
                    self.slice = None;
                }
            } else {
                return None;
            }
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::{DeltaDecoder, DeltaEncoder, DeltaSliceDecoder};

    proptest::prop_compose! {
        pub fn arb_delta_vector()(bv in proptest::collection::vec(proptest::arbitrary::any::<u64>(), 0..256)) -> Vec<u64> {
            bv
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_u64_slice(dv in arb_delta_vector()) {
            let mut encoder = DeltaEncoder::default();
            for x in dv.iter() {
                encoder.push(*x).unwrap();
            }
            let decoder = DeltaSliceDecoder::new(encoder.as_ref()).unwrap();
            let encoded = dv;
            let decoded = decoder.collect::<Vec<_>>();
            assert_eq!(encoded.len(), decoded.len());
            for (idx, (e, d)) in std::iter::zip(encoded, decoded).enumerate() {
                let d = d.unwrap();
                assert_eq!(e, d, "idx={idx}");
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_u64(dv in arb_delta_vector()) {
            let mut encoder = DeltaEncoder::default();
            for x in dv.iter() {
                encoder.push(*x).unwrap();
            }
            let decoder = DeltaDecoder::new(encoder.as_ref());
            let encoded = dv;
            let decoded = decoder.collect::<Vec<_>>();
            assert_eq!(encoded.len(), decoded.len());
            for (idx, (e, d)) in std::iter::zip(encoded, decoded).enumerate() {
                let d = d.unwrap();
                assert_eq!(e, d, "idx={idx}");
            }
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_u64_concat(dv1 in arb_delta_vector(), dv2 in arb_delta_vector()) {
            let mut encoder1 = DeltaEncoder::default();
            for x in dv1.iter() {
                encoder1.push(*x).unwrap();
            }
            let mut encoder2 = DeltaEncoder::default();
            for x in dv2.iter() {
                encoder2.push(*x).unwrap();
            }
            let mut encoded: Vec<u8> = vec![];
            encoded.extend(encoder1.as_ref());
            encoded.extend(encoder2.as_ref());
            let decoder = DeltaDecoder::new(encoded.as_ref());
            let mut encoded = dv1;
            encoded.extend(dv2.iter());
            let decoded = decoder.collect::<Vec<_>>();
            assert_eq!(encoded.len(), decoded.len());
            for (idx, (e, d)) in std::iter::zip(encoded, decoded).enumerate() {
                let d = d.unwrap();
                assert_eq!(e, d, "idx={idx}");
            }
        }
    }
}
