use std::fmt::Debug;

use buffertk::{v64, Packable};

use prototk::FieldNumber;

mod combine7;
mod iter7;
mod ordered;

use combine7::Combine7BitChunks;
use iter7::Iterate7BitChunks;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    CouldNotExtend {
        field_number: u32,
    },
    NotValidUtf8,
}

///////////////////////////////////////////// DataType /////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataType {
    Fixed32,
    Fixed64,
    SFixed32,
    SFixed64,
    Bytes,
    Bytes16,
    Bytes32,
    String,
    Message,
}

impl DataType {
    fn discriminant(&self) -> u64 {
        match self {
            Self::Fixed32 => 1,
            Self::Fixed64 => 2,
            Self::SFixed32 => 3,
            Self::SFixed64 => 4,
            Self::Bytes => 5,
            Self::Bytes16 => 6,
            Self::Bytes32 => 7,
            Self::String => 8,
            Self::Message => 15,
        }
    }

    fn from_discriminant(x: u64) -> Option<Self> {
        match x {
            1 => Some(Self::Fixed32),
            2 => Some(Self::Fixed64),
            3 => Some(Self::SFixed32),
            4 => Some(Self::SFixed64),
            5 => Some(Self::Bytes),
            6 => Some(Self::Bytes16),
            7 => Some(Self::Bytes32),
            8 => Some(Self::String),
            15 => Some(Self::Message),
            _ => None,
        }
    }
}

///////////////////////////////////////////// TupleKey /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TupleKey {
    buf: Vec<u8>,
}

impl TupleKey {
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn append(&mut self, other: &mut TupleKey) {
        self.buf.append(&mut other.buf);
    }

    pub fn extend(&mut self, f: FieldNumber, value: DataType) {
        self.extend_field_number(f, value);
    }

    pub fn extend_with_key<E: Element>(&mut self, f: FieldNumber, elem: E, value: DataType) {
        self.extend_field_number(f, value);
        elem.append_to(self);
    }

    fn append_bytes(&mut self, iter: Iterate7BitChunks) {
        for c in iter {
            self.buf.push(c)
        }
    }

    fn from_field_number(f: FieldNumber, value: DataType) -> ([u8; 10], usize) {
        let f: v64 = v64::from(((f.get() as u64) << 4) | value.discriminant());
        let mut buf = [0u8; 10];
        let sz = f.pack_sz();
        v64::pack(&f, &mut buf[0..sz]);
        buf[0..sz].iter_mut().for_each(|c| *c = c.rotate_left(1));
        (buf, sz)
    }

    fn extend_field_number(&mut self, f: FieldNumber, value: DataType) {
        let (buf, sz) = Self::from_field_number(f, value);
        self.buf.extend_from_slice(&buf[0..sz])
    }
}

///////////////////////////////////////// TupleKeyIterator /////////////////////////////////////////

pub struct TupleKeyIterator<'a> {
    tk: &'a TupleKey,
    offset: usize,
}

impl<'a> TupleKeyIterator<'a> {
    pub fn new(tk: &'a TupleKey) -> Self {
        Self {
            tk,
            offset: 0,
        }
    }
}

impl<'a> Iterator for TupleKeyIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.tk.buf.len() {
            None
        } else {
            let start = self.offset;
            while self.offset < self.tk.buf.len() && self.tk.buf[self.offset] & 0x1 != 0 {
                self.offset += 1;
            }
            if self.offset < self.tk.buf.len() {
                self.offset += 1;
            }
            let limit = self.offset;
            Some(&self.tk.buf[start..limit])
        }
    }
}

////////////////////////////////////////// TupleKeyParser //////////////////////////////////////////

pub struct TupleKeyParser<'a> {
    iter: TupleKeyIterator<'a>,
}

impl<'a> TupleKeyParser<'a> {
    pub fn new(tk: &'a TupleKey) -> Self {
        Self {
            iter: TupleKeyIterator::new(tk),
        }
    }

    pub fn extend(&mut self, f: FieldNumber, value: DataType) -> Result<(), &'static str> {
        let elem = match self.iter.next() {
            Some(elem) => elem,
            None => { return Err("no more elements to TupleKey"); },
        };
        let (buf, sz) = TupleKey::from_field_number(f, value);
        if &buf[0..sz] != elem {
            return Err("tag does not match");
        }
        Ok(())
    }

    pub fn extend_with_key<E: Element>(&mut self, f: FieldNumber, value: DataType) -> Result<E, &'static str> {
        let elem = match self.iter.next() {
            Some(elem) => elem,
            None => {
                return Err("no more elements to TupleKey");
            }
        };
        // Pack the tuple key and then compare.
        let (buf, sz) = TupleKey::from_field_number(f, value);
        if &buf[0..sz] != elem {
            return Err("tag does not match");
        }
        // Check the discriminant
        let discriminant = match self.iter.next() {
            Some(discriminant) => discriminant,
            None => {
                return Err("no more elements to TupleKey");
            }
        };
        if discriminant.len() > 1 {
            return Err("discriminant has too many bytes");
        }
        let data_type = DataType::from_discriminant(discriminant[0] as u64 >> 1);
        if Some(E::DATA_TYPE) != data_type {
            return Err("key is of wrong type");
        }
        // Read the value
        let value = match self.iter.next() {
            Some(value) => value,
            None => {
                if E::VARIABLE_VALUE_CAN_BE_EMPTY {
                    &[]
                } else {
                    return Err("could not find value element");
                }
            }
        };
        E::parse_from(value)
    }
}

////////////////////////////////////////////// Element /////////////////////////////////////////////

pub trait Element: Sized {
    const DATA_TYPE: DataType;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool;

    fn append_to(&self, key: &mut TupleKey);
    fn parse_from(buf: &[u8]) -> Result<Self, &'static str>;
}

impl Element for u32 {
    const DATA_TYPE: DataType = DataType::Fixed32;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = false;

    fn append_to(&self, key: &mut TupleKey) {
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.buf.push(((self >> 24) | 1) as u8);
        key.buf.push(((self >> 17) | 1) as u8);
        key.buf.push(((self >> 10) | 1) as u8);
        key.buf.push(((self >> 3) | 1) as u8);
        key.buf.push(((self & 0xf) << 4) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 5 {
            return Err("buf not exactly 5 bytes");
        }
        let mut key = 0u32;
        key |= ((buf[0] & 0xfe) as u32) << 24;
        key |= ((buf[1] & 0xfe) as u32) << 17;
        key |= ((buf[2] & 0xfe) as u32) << 10;
        key |= ((buf[3] & 0xfe) as u32) << 3;
        key |= ((buf[4] & 0xf0) as u32) >> 4;
        Ok(key)
    }
}

impl Element for u64 {
    const DATA_TYPE: DataType = DataType::Fixed64;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = false;

    fn append_to(&self, key: &mut TupleKey) {
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.buf.push(((self >> 56) | 1) as u8);
        key.buf.push(((self >> 49) | 1) as u8);
        key.buf.push(((self >> 42) | 1) as u8);
        key.buf.push(((self >> 35) | 1) as u8);
        key.buf.push(((self >> 28) | 1) as u8);
        key.buf.push(((self >> 21) | 1) as u8);
        key.buf.push(((self >> 14) | 1) as u8);
        key.buf.push(((self >> 7) | 1) as u8);
        key.buf.push(((self >> 0) | 1) as u8);
        key.buf.push(((self & 0x1) << 7) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 10 {
            return Err("buf not exactly 10 bytes");
        }
        let mut key = 0u64;
        key |= ((buf[0] & 0xfe) as u64) << 56;
        key |= ((buf[1] & 0xfe) as u64) << 49;
        key |= ((buf[2] & 0xfe) as u64) << 42;
        key |= ((buf[3] & 0xfe) as u64) << 35;
        key |= ((buf[4] & 0xfe) as u64) << 28;
        key |= ((buf[5] & 0xfe) as u64) << 21;
        key |= ((buf[6] & 0xfe) as u64) << 14;
        key |= ((buf[7] & 0xfe) as u64) << 7;
        key |= ((buf[8] & 0xfe) as u64) << 0;
        key |= ((buf[9] & 0x80) as u64) >> 7;
        Ok(key)
    }
}

impl Element for i32 {
    const DATA_TYPE: DataType = DataType::SFixed32;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = false;

    fn append_to(&self, key: &mut TupleKey) {
        let num: u32 = ordered::encode_i32(*self);
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.buf.push(((num >> 24) | 1) as u8);
        key.buf.push(((num >> 17) | 1) as u8);
        key.buf.push(((num >> 10) | 1) as u8);
        key.buf.push(((num >> 3) | 1) as u8);
        key.buf.push(((num & 0xf) << 4) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 5 {
            return Err("buf not exactly 5 bytes");
        }
        let mut key = 0u32;
        key |= ((buf[0] & 0xfe) as u32) << 24;
        key |= ((buf[1] & 0xfe) as u32) << 17;
        key |= ((buf[2] & 0xfe) as u32) << 10;
        key |= ((buf[3] & 0xfe) as u32) << 3;
        key |= ((buf[4] & 0xf0) as u32) >> 4;
        Ok(ordered::decode_i32(key))
    }
}

impl Element for i64 {
    const DATA_TYPE: DataType = DataType::SFixed64;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = false;

    fn append_to(&self, key: &mut TupleKey) {
        let num: u64 = ordered::encode_i64(*self);
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.buf.push(((num >> 56) | 1) as u8);
        key.buf.push(((num >> 49) | 1) as u8);
        key.buf.push(((num >> 42) | 1) as u8);
        key.buf.push(((num >> 35) | 1) as u8);
        key.buf.push(((num >> 28) | 1) as u8);
        key.buf.push(((num >> 21) | 1) as u8);
        key.buf.push(((num >> 14) | 1) as u8);
        key.buf.push(((num >> 7) | 1) as u8);
        key.buf.push(((num >> 0) | 1) as u8);
        key.buf.push(((num & 0x1) << 7) as u8);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 10 {
            return Err("buf not exactly 10 bytes");
        }
        let mut key = 0u64;
        key |= ((buf[0] & 0xfe) as u64) << 56;
        key |= ((buf[1] & 0xfe) as u64) << 49;
        key |= ((buf[2] & 0xfe) as u64) << 42;
        key |= ((buf[3] & 0xfe) as u64) << 35;
        key |= ((buf[4] & 0xfe) as u64) << 28;
        key |= ((buf[5] & 0xfe) as u64) << 21;
        key |= ((buf[6] & 0xfe) as u64) << 14;
        key |= ((buf[7] & 0xfe) as u64) << 7;
        key |= ((buf[8] & 0xfe) as u64) << 0;
        key |= ((buf[9] & 0x80) as u64) >> 7;
        Ok(ordered::decode_i64(key))
    }
}

impl Element for Vec<u8> {
    const DATA_TYPE: DataType = DataType::Bytes;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = true;

    fn append_to(&self, key: &mut TupleKey) {
        let iter = Iterate7BitChunks::new(self);
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.append_bytes(iter);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        let combiner = Combine7BitChunks::new(buf);
        Ok(combiner.collect())
    }
}

impl Element for [u8; 16] {
    const DATA_TYPE: DataType = DataType::Bytes16;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = false;

    fn append_to(&self, key: &mut TupleKey) {
        let bytes: &[u8] = self;
        let iter = Iterate7BitChunks::new(bytes);
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.append_bytes(iter);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 19 {
            return Err("invalid length for 16-byte array");
        }
        let combiner = Combine7BitChunks::new(buf);
        let mut ret = [0u8; 16];
        for (idx, byte) in combiner.enumerate() {
            ret[idx] = byte;
        }
        Ok(ret)
    }
}

impl Element for [u8; 32] {
    const DATA_TYPE: DataType = DataType::Bytes32;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = false;

    fn append_to(&self, key: &mut TupleKey) {
        let bytes: &[u8] = self;
        let iter = Iterate7BitChunks::new(bytes);
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.append_bytes(iter);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 37 {
            return Err("invalid length for 16-byte array");
        }
        let combiner = Combine7BitChunks::new(buf);
        let mut ret = [0u8; 32];
        for (idx, byte) in combiner.enumerate() {
            ret[idx] = byte;
        }
        Ok(ret)
    }
}

impl Element for String {
    const DATA_TYPE: DataType = DataType::String;
    const VARIABLE_VALUE_CAN_BE_EMPTY: bool = true;

    fn append_to(&self, key: &mut TupleKey) {
        let iter = Iterate7BitChunks::new(self.as_bytes());
        let discriminant = Self::DATA_TYPE.discriminant();
        assert!(discriminant < 16);
        key.buf.push((discriminant << 1) as u8);
        key.append_bytes(iter);
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        let combiner = Combine7BitChunks::new(buf);
        String::from_utf8(combiner.collect()).map_err(|_| "invalid UTF-8 sequence")
    }
}

///////////////////////////////////////// FromIntoTupleKey /////////////////////////////////////////

pub trait FromIntoTupleKey {
    fn from_tuple_key(tk: &TupleKey) -> Result<Self, Error> where Self: Sized;
    fn into_tuple_key(self) -> TupleKey;
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tuple_key {
    use super::*;

    mod append {
        use super::*;

        #[test]
        fn two_empties() {
            let mut tk1 = TupleKey::default();
            let mut tk2 = TupleKey::default();
            tk1.append(&mut tk2);
            assert!(tk1.as_bytes().is_empty());
            assert!(tk2.is_empty());
        }

        #[test]
        fn two_triplets() {
            let mut tk1 = TupleKey::default();
            tk1.extend_with_key(FieldNumber::must(1), "A".to_owned(), DataType::Message);
            tk1.extend_with_key(FieldNumber::must(2), "B".to_owned(), DataType::Message);
            tk1.extend_with_key(FieldNumber::must(3), "C".to_owned(), DataType::Message);
            let mut tk2 = TupleKey::default();
            tk2.extend_with_key(FieldNumber::must(4), "D".to_owned(), DataType::Message);
            tk2.extend_with_key(FieldNumber::must(5), "E".to_owned(), DataType::Message);
            tk2.extend_with_key(FieldNumber::must(6), "F".to_owned(), DataType::Message);
            // preconditions
            assert_eq!(&[62, 16, 65, 128, 94, 16, 67, 0, 126, 16, 67, 128], tk1.as_bytes());
            assert_eq!(&[158, 16, 69, 0, 190, 16, 69, 128, 222, 16, 71, 0], tk2.as_bytes());
            // what we want to test
            tk1.append(&mut tk2);
            assert_eq!(&[62, 16, 65, 128, 94, 16, 67, 0, 126, 16, 67, 128, 158, 16, 69, 0, 190, 16, 69, 128, 222, 16, 71, 0], tk1.as_bytes());
            assert!(tk2.is_empty());
        }
    }

    mod extend {
        use super::*;

        #[test]
        fn field_7() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(7), DataType::Message);
            // 0b11111110
            assert_eq!(&[254], tk.as_bytes());
        }

        #[test]
        fn field_8() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(8), DataType::Message);
            // 0b00011111
            // 0b00000010
            assert_eq!(&[31, 2], tk.as_bytes());
        }

        #[test]
        fn field_1023() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(1023), DataType::Message);
            // 0b11111111
            // 0b11111110
            assert_eq!(&[0xff, 0xfe], tk.as_bytes());
        }

        #[test]
        fn field_1024() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(1024), DataType::Message);
            // 0b00011111
            // 0b00000001
            // 0b00000010
            assert_eq!(&[31, 1, 2], tk.as_bytes());
        }

        #[test]
        fn field_131071() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(131071), DataType::Message);
            // 0b11111111
            // 0b11111111
            // 0b11111110
            assert_eq!(&[0xff, 0xff, 0xfe], tk.as_bytes());
        }

        #[test]
        fn field_131072() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(131072), DataType::Message);
            // 0b00011111
            // 0b00000001
            // 0b00000001
            // 0b00000010
            assert_eq!(&[31, 1, 1, 2], tk.as_bytes());
        }

        #[test]
        fn field_16777215() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(16777215), DataType::Message);
            // 0b11111111
            // 0b11111111
            // 0b11111111
            // 0b11111110
            assert_eq!(&[0xff, 0xff, 0xff, 0xfe], tk.as_bytes());
        }

        #[test]
        fn field_16777216() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(16777216), DataType::Message);
            // 0b00011111
            // 0b00000001
            // 0b00000001
            // 0b00000001
            // 0b00000010
            assert_eq!(&[31, 1, 1, 1, 2], tk.as_bytes());
        }

        #[test]
        fn field_536870911() {
            let mut tk = TupleKey::default();
            tk.extend(FieldNumber::must(536870911), DataType::Message);
            // 0b11111111
            // 0b11111111
            // 0b11111111
            // 0b11111111
            // 0b00111110
            assert_eq!(&[0xff, 0xff, 0xff, 0xff, 0x3e], tk.as_bytes());
        }
    }

    mod extend_with_key {
        use super::*;

        fn extend<X: Element + Clone + Debug + Eq>(f: FieldNumber, x: X, bytes: &[u8]) {
            // Check the serialization path.
            let mut tk = TupleKey::default();
            tk.extend_with_key(f, x.clone(), DataType::Message);
            assert_eq!(bytes, tk.as_bytes());
            // Deserialize
            let mut parser = TupleKeyParser::new(&tk);
            let got = parser.extend_with_key::<X>(f, DataType::Message).unwrap();
            assert_eq!(x, got);
        }

        #[test]
        fn extend_u32() {
            extend(FieldNumber::must(7), 0u32, &[0xfe, 0x02, 0x01, 0x01, 0x01, 0x01, 0x00]);
            extend(FieldNumber::must(7), 1u32, &[0xfe, 0x02, 0x01, 0x01, 0x01, 0x01, 0x10]);
            extend(FieldNumber::must(7), 0xcafef00du32, &[0xfe, 0x02, 0xcb, 0x7f, 0xbd, 0x01, 0xd0]);
            extend(FieldNumber::must(7), u32::max_value() - 1, &[0xfe, 0x02, 0xff, 0xff, 0xff, 0xff, 0xe0]);
            extend(FieldNumber::must(7), u32::max_value(), &[0xfe, 0x02, 0xff, 0xff, 0xff, 0xff, 0xf0]);
        }

        #[test]
        fn extend_u64() {
            extend(FieldNumber::must(7), 0u64, &[0xfe, 0x04, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0]);
            extend(FieldNumber::must(7), 1u64, &[0xfe, 0x04, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x80]);
            extend(FieldNumber::must(7), 0xcafef00d00c0ffeeu64, &[0xfe, 0x04, 0xcb, 0x7f, 0xbd, 0x01, 0xd1, 0x07, 0x03, 0xff, 0xef, 0x00]);
            extend(FieldNumber::must(7), u64::max_value() - 1, &[0xfe, 0x04, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
            extend(FieldNumber::must(7), u64::max_value(), &[0xfe, 0x04, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x80]);
        }

        #[test]
        fn extend_i32() {
            extend(FieldNumber::must(7), i32::min_value(), &[0xfe, 0x06, 0x01, 0x01, 0x01, 0x01, 0]);
            extend(FieldNumber::must(7), i32::min_value() + 1, &[0xfe, 0x06, 0x01, 0x01, 0x01, 0x01, 0x10]);
            extend(FieldNumber::must(7), -1i32, &[0xfe, 0x06, 0x7f, 0xff, 0xff, 0xff, 0xf0]);
            extend(FieldNumber::must(7), 0i32, &[0xfe, 0x06, 0x81, 0x01, 0x01, 0x01, 0x00]);
            extend(FieldNumber::must(7), 1i32, &[0xfe, 0x06, 0x81, 0x01, 0x01, 0x01, 0x10]);
            extend(FieldNumber::must(7), 0x1eaff00di32, &[0xfe, 0x06, 0x9f, 0x57, 0xfd, 0x01, 0xd0]);
            extend(FieldNumber::must(7), i32::max_value() - 1, &[0xfe, 0x06, 0xff, 0xff, 0xff, 0xff, 0xe0]);
            extend(FieldNumber::must(7), i32::max_value(), &[0xfe, 0x06, 0xff, 0xff, 0xff, 0xff, 0xf0]);
        }


        #[test]
        fn extend_i64() {
            extend(FieldNumber::must(7), i64::min_value(), &[0xfe, 0x08, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x1, 0]);
            extend(FieldNumber::must(7), i64::min_value() + 1, &[0xfe, 0x08, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x1, 0x01, 0x80]);
            extend(FieldNumber::must(7), -1i64, &[0xfe, 0x08, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x80]);
            extend(FieldNumber::must(7), 0i64, &[0xfe, 0x08, 0x81, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0]);
            extend(FieldNumber::must(7), 1i64, &[0xfe, 0x08, 0x81, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x80]);
            extend(FieldNumber::must(7), 0x1eaff00d00c0ffeei64, &[0xfe, 0x08, 0x9f, 0x57, 0xfd, 0x01, 0xd1, 0x07, 0x03, 0xff, 0xef, 0x00]);
            extend(FieldNumber::must(7), i64::max_value() - 1, &[0xfe, 0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0]);
            extend(FieldNumber::must(7), i64::max_value(), &[0xfe, 0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x80]);
        }

        #[test]
        fn extend_bytes() {
            let exp: Vec<u8> = vec![];
            extend(FieldNumber::must(7), exp, &[0xfe, 10]);
            let exp: Vec<u8> = vec![0xc0];
            extend(FieldNumber::must(7), exp, &[0xfe, 10, 0xc1, 0]);
            let exp: Vec<u8> = vec![0xc0, 0xff];
            extend(FieldNumber::must(7), exp, &[0xfe, 10, 0xc1, 0x7f, 0xc0]);
            let exp: Vec<u8> = vec![0xc0, 0xff, 0xee];
            extend(FieldNumber::must(7), exp, &[0xfe, 10, 0xc1, 0x7f, 0xfb, 0xc0]);
        }
    }

    mod iterator {
        use super::*;

        #[test]
        fn empty() {
            let tk1 = TupleKey::default();
            let mut iter = TupleKeyIterator::new(&tk1);
            assert_eq!(None, iter.next());
        }

        #[test]
        fn abc() {
            let mut tk1 = TupleKey::default();
            tk1.extend_with_key(FieldNumber::must(1), "A".to_string(), DataType::Message);
            tk1.extend_with_key(FieldNumber::must(2), "B".to_string(), DataType::Message);
            tk1.extend_with_key(FieldNumber::must(3), "C".to_string(), DataType::Message);
            let mut iter = TupleKeyIterator::new(&tk1);
            let buf: &[u8] = &[62];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[16];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[65, 128];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[94];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[16];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[67, 0];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[126];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[16];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[67, 128];
            assert_eq!(Some(buf), iter.next());
            assert_eq!(None, iter.next());
        }
    }
}
