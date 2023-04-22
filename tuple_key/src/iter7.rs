///////////////////////////////////////// Iterate7BitChunks ////////////////////////////////////////

pub struct Iterate7BitChunks<'a> {
    bytes: &'a [u8],
    offset: usize,
    remains: u64,
    remains_bits: usize,
}

impl<'a> Iterate7BitChunks<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            remains: 0,
            remains_bits: 0,
        }
    }
}

impl<'a> Iterator for Iterate7BitChunks<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.remains_bits > 7 {
            let x = (self.remains >> (self.remains_bits - 7)) as u8 & 0x7f;
            self.remains_bits -= 7;
            Some((x << 1) | 1)
        } else if self.offset < self.bytes.len() {
            self.remains <<= 8;
            self.remains |= self.bytes[self.offset] as u64;
            self.offset += 1;
            self.remains_bits += 8;
            self.next()
        } else if self.remains_bits > 0 {
            assert!(self.remains_bits <= 7);
            let mut x: u8 = self.remains as u8;
            x <<= 8 - self.remains_bits;
            self.remains_bits = 0;
            Some(x)
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
        let mut iter = Iterate7BitChunks::new(&[]);
        assert_eq!(None, iter.next());
    }

    #[test]
    fn one_byte() {
        let mut iter = Iterate7BitChunks::new(&[129]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0x80), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn two_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x40), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn three_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65, 33]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x20), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn four_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65, 33, 17]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x23), iter.next());
        assert_eq!(Some(0x10), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn five_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65, 33, 17, 9]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x23), iter.next());
        assert_eq!(Some(0x11), iter.next());
        assert_eq!(Some(0x48), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn six_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65, 33, 17, 9, 5]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x23), iter.next());
        assert_eq!(Some(0x11), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x14), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn seven_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65, 33, 17, 9, 5, 3]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x23), iter.next());
        assert_eq!(Some(0x11), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x15), iter.next());
        assert_eq!(Some(0x6), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn eight_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65, 33, 17, 9, 5, 3, 2]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x23), iter.next());
        assert_eq!(Some(0x11), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x15), iter.next());
        assert_eq!(Some(0x07), iter.next());
        assert_eq!(Some(0x03), iter.next());
        assert_eq!(Some(0x00), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn nine_bytes() {
        let mut iter = Iterate7BitChunks::new(&[129, 65, 33, 17, 9, 5, 3, 2, 1]);
        assert_eq!(Some(0x81), iter.next());
        assert_eq!(Some(0xa1), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x23), iter.next());
        assert_eq!(Some(0x11), iter.next());
        assert_eq!(Some(0x49), iter.next());
        assert_eq!(Some(0x15), iter.next());
        assert_eq!(Some(0x07), iter.next());
        assert_eq!(Some(0x03), iter.next());
        assert_eq!(Some(0x01), iter.next());
        assert_eq!(Some(0x40), iter.next());
        assert_eq!(None, iter.next());
    }
}
