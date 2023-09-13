///////////////////////////////////////// Combine7BitChunks ////////////////////////////////////////

pub struct Combine7BitChunks<'a> {
    bytes: &'a [u8],
    offset: usize,
    remains: u64,
    remains_bits: usize,
}

impl<'a> Combine7BitChunks<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            remains: 0,
            remains_bits: 0,
        }
    }
}

impl<'a> Iterator for Combine7BitChunks<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        while self.offset < self.bytes.len() && self.remains_bits < 8 {
            self.remains <<= 7;
            self.remains |= (self.bytes[self.offset] >> 1) as u64;
            self.remains_bits += 7;
            self.offset += 1;
        }
        if self.remains_bits >= 8 {
            let ret = (self.remains >> (self.remains_bits - 8)) as u8;
            self.remains_bits -= 8;
            Some(ret)
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
    fn empty() {
        let mut iter = Combine7BitChunks::new(&[]);
        assert_eq!(None, iter.next());
    }

    #[test]
    fn one_byte() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b10000000]);
        assert_eq!(Some(1), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b00000011, 0b00000000]);
        assert_eq!(Some(2), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b00000011, 0b10000000]);
        assert_eq!(Some(3), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b00000101, 0b00000000]);
        assert_eq!(Some(4), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b00000000]);
        assert_eq!(Some(254), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b10000000]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn two_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11000000]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn three_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11111111, 0b11100000]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn four_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11110000]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn five_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111000]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn six_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111100]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn seven_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111110]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn eight_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b10000000]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn nine_bytes() {
        let mut iter = Combine7BitChunks::new(&[0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000001, 0b00000000]);
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(Some(0), iter.next());
        assert_eq!(None, iter.next());

        let mut iter = Combine7BitChunks::new(&[0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11111111, 0b11000000]);
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(Some(255), iter.next());
        assert_eq!(None, iter.next());
    }
}
