use std::fmt::Debug;

use buffertk::{v64, Packable};

use prototk::FieldNumber;
use prototk_derive::Message;

use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

mod combine7;
mod iter7;
mod ordered;

use combine7::Combine7BitChunks;
use iter7::Iterate7BitChunks;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
pub enum Error {
    #[prototk(311296, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(311297, message)]
    CouldNotExtend {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint32)]
        field_number: u32,
    },
    #[prototk(311298, message)]
    UnpackError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        err: prototk::Error,
    },
    #[prototk(311299, message)]
    NotValidUtf8 {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(311300, message)]
    InvalidTag {
        #[prototk(1, message)]
        core: ErrorCore,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::Success { core, .. } => core,
            Error::CouldNotExtend { core, .. } => core,
            Error::UnpackError { core, .. } => core,
            Error::NotValidUtf8 { core, .. } => core,
            Error::InvalidTag { core, .. } => core,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::Success { core, .. } => core,
            Error::CouldNotExtend { core, .. } => core,
            Error::UnpackError { core, .. } => core,
            Error::NotValidUtf8 { core, .. } => core,
            Error::InvalidTag { core, .. } => core,
        }
    }
}

impl Default for Error {
    fn default() -> Error {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

impl From<prototk::Error> for Error {
    fn from(err: prototk::Error) -> Self {
        Self::UnpackError {
            core: ErrorCore::default(),
            err,
        }
    }
}

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        // TODO(rescrv): put a one-line error as first line.
        self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.core_mut().set_token(identifier, value);
        self
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.core_mut().set_url(identifier, url);
        self
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error
    where
        X: Debug,
    {
        self.core_mut().set_variable(variable, x);
        self
    }
}

iotoz! {Error}

///////////////////////////////////////////// DataType /////////////////////////////////////////////

// NOTE(rescrv): Enums always take type message for future extensibility.
#[derive(Clone, Debug, Default, Message, Eq, PartialEq)]
#[allow(non_camel_case_types)]
pub enum DataType {
    #[default]
    #[prototk(1, message)]
    unit,
    #[prototk(2, message)]
    int32,
    #[prototk(3, message)]
    int64,
    #[prototk(4, message)]
    uint32,
    #[prototk(5, message)]
    uint64,
    #[prototk(6, message)]
    sint32,
    #[prototk(7, message)]
    sint64,
    #[prototk(8, message)]
    fixed32,
    #[prototk(9, message)]
    fixed64,
    #[prototk(10, message)]
    sfixed32,
    #[prototk(11, message)]
    sfixed64,
    #[prototk(12, message)]
    float,
    #[prototk(13, message)]
    double,
    #[prototk(14, message)]
    Bool,
    #[prototk(15, message)]
    bytes,
    #[prototk(16, message)]
    bytes16,
    #[prototk(17, message)]
    bytes32,
    #[prototk(18, message)]
    bytes64,
    #[prototk(19, message)]
    string,
    #[prototk(20, message)]
    message,
}

impl DataType {
    fn discriminant(&self) -> u64 {
        match self {
            DataType::unit => 0,
            DataType::fixed32 => 1,
            DataType::fixed64 => 2,
            DataType::sfixed32 => 3,
            DataType::sfixed64 => 4,
            DataType::bytes => 5,
            DataType::bytes16 => 6,
            DataType::bytes32 => 7,
            DataType::string => 8,
            DataType::message => 15,
            _ => 16,
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

    pub fn extend(&mut self, f: FieldNumber) {
        self.extend_field_number(f, DataType::unit);
        ().append_to(self);
    }

    pub fn extend_with_key<E: Element>(&mut self, f: FieldNumber, elem: E) {
        self.extend_field_number(f, E::DATA_TYPE);
        elem.append_to(self);
    }

    fn append_bytes(&mut self, iter: impl Iterator<Item=u8>) -> usize {
        let mut count = 0;
        for c in iter {
            self.buf.push(c);
            count += 1;
        }
        count
    }

    fn from_field_number(f: FieldNumber, value: DataType) -> ([u8; 10], usize) {
        assert!(value.discriminant() < 16);
        let f: v64 = v64::from(((f.get() as u64) << 4) | value.discriminant());
        let mut buf = [0u8; 10];
        let sz = f.pack_sz();
        v64::pack(&f, &mut buf[0..sz]);
        // Shift the high order bit of the varint to the low order bit of the varint
        buf[0..sz].iter_mut().for_each(|c| *c = c.rotate_left(1));
        (buf, sz)
    }

    fn extend_field_number(&mut self, f: FieldNumber, value: DataType) {
        let (buf, sz) = Self::from_field_number(f, value);
        self.buf.extend_from_slice(&buf[0..sz])
    }
}

///////////////////////////////////////// TupleKeyIterator /////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct TupleKeyIterator<'a> {
    tk: &'a TupleKey,
    offset: usize,
}

impl<'a> TupleKeyIterator<'a> {
    pub fn new(tk: &'a TupleKey) -> Self {
        Self { tk, offset: 0 }
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

    pub fn extend(&mut self, f: FieldNumber) -> Result<(), &'static str> {
        self.extend_tag(f, DataType::unit)?;
        let pad = match self.iter.next() {
            Some(pad) => pad,
            None => {
                return Err("no more elements to TupleKey");
            }
        };
        if pad.len() != 1 {
            return Err("unit struct with length != 1");
        }
        Ok(())
    }

    pub fn extend_with_key<E: Element>(
        &mut self,
        f: FieldNumber,
    ) -> Result<E, &'static str> {
        // First we extend as normal.
        self.extend_tag(f, E::DATA_TYPE)?;
        // Read the value
        let value = match self.iter.next() {
            Some(value) => value,
            None => {
                return Err("missing value element");
            }
        };
        E::parse_from(value)
    }

    fn extend_tag(&mut self, f: FieldNumber, ty: DataType) -> Result<(), &'static str> {
        let elem = match self.iter.next() {
            Some(elem) => elem,
            None => {
                return Err("no more elements to TupleKey");
            }
        };
        let (buf, sz) = TupleKey::from_field_number(f, ty);
        if &buf[0..sz] != elem {
            return Err("tag does not match");
        }
        Ok(())
    }
}

////////////////////////////////////////////// Element /////////////////////////////////////////////

pub trait Element: Sized {
    const DATA_TYPE: DataType;

    fn append_to(&self, key: &mut TupleKey);
    fn parse_from(buf: &[u8]) -> Result<Self, &'static str>;
}

impl Element for () {
    const DATA_TYPE: DataType = DataType::unit;

    fn append_to(&self, key: &mut TupleKey) {
        key.append_bytes(&mut [0u8].iter().copied());
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() != 1 {
            return Err("unit not exactly 1 bytes");
        }
        Ok(())
    }
}

impl Element for u32 {
    const DATA_TYPE: DataType = DataType::fixed32;

    fn append_to(&self, key: &mut TupleKey) {
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
    const DATA_TYPE: DataType = DataType::fixed64;

    fn append_to(&self, key: &mut TupleKey) {
        key.buf.push(((self >> 56) | 1) as u8);
        key.buf.push(((self >> 49) | 1) as u8);
        key.buf.push(((self >> 42) | 1) as u8);
        key.buf.push(((self >> 35) | 1) as u8);
        key.buf.push(((self >> 28) | 1) as u8);
        key.buf.push(((self >> 21) | 1) as u8);
        key.buf.push(((self >> 14) | 1) as u8);
        key.buf.push(((self >> 7) | 1) as u8);
        key.buf.push((self | 1) as u8);
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
        key |= (buf[8] & 0xfe) as u64;
        key |= ((buf[9] & 0x80) as u64) >> 7;
        Ok(key)
    }
}

impl Element for i32 {
    const DATA_TYPE: DataType = DataType::sfixed32;

    fn append_to(&self, key: &mut TupleKey) {
        let num: u32 = ordered::encode_i32(*self);
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
    const DATA_TYPE: DataType = DataType::sfixed64;

    fn append_to(&self, key: &mut TupleKey) {
        let num: u64 = ordered::encode_i64(*self);
        key.buf.push(((num >> 56) | 1) as u8);
        key.buf.push(((num >> 49) | 1) as u8);
        key.buf.push(((num >> 42) | 1) as u8);
        key.buf.push(((num >> 35) | 1) as u8);
        key.buf.push(((num >> 28) | 1) as u8);
        key.buf.push(((num >> 21) | 1) as u8);
        key.buf.push(((num >> 14) | 1) as u8);
        key.buf.push(((num >> 7) | 1) as u8);
        key.buf.push((num | 1) as u8);
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
        key |= (buf[8] & 0xfe) as u64;
        key |= ((buf[9] & 0x80) as u64) >> 7;
        Ok(ordered::decode_i64(key))
    }
}

impl Element for Vec<u8> {
    const DATA_TYPE: DataType = DataType::bytes;

    fn append_to(&self, key: &mut TupleKey) {
        let iter = Iterate7BitChunks::new(self);
        if key.append_bytes(iter) == 0 {
            key.append_bytes(&mut [0u8].iter().copied());
        }
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() == 1 {
            return Ok(Vec::new());
        }
        let combiner = Combine7BitChunks::new(buf);
        Ok(combiner.collect())
    }
}

impl Element for [u8; 16] {
    const DATA_TYPE: DataType = DataType::bytes16;

    fn append_to(&self, key: &mut TupleKey) {
        let bytes: &[u8] = self;
        let iter = Iterate7BitChunks::new(bytes);
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
    const DATA_TYPE: DataType = DataType::bytes32;

    fn append_to(&self, key: &mut TupleKey) {
        let bytes: &[u8] = self;
        let iter = Iterate7BitChunks::new(bytes);
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
    const DATA_TYPE: DataType = DataType::string;

    fn append_to(&self, key: &mut TupleKey) {
        let iter = Iterate7BitChunks::new(self.as_bytes());
        if key.append_bytes(iter) == 0 {
            key.append_bytes(&mut [0u8].iter().copied());
        }
    }

    fn parse_from(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() == 1 {
            return Ok(String::new());
        }
        let combiner = Combine7BitChunks::new(buf);
        String::from_utf8(combiner.collect()).map_err(|_| "invalid UTF-8 sequence")
    }
}

/////////////////////////////////////////// TypedTupleKey //////////////////////////////////////////

pub trait TypedTupleKey: TryFrom<TupleKey> + Into<TupleKey> {}

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
            tk1.extend_with_key(FieldNumber::must(1), "A".to_owned());
            tk1.extend_with_key(FieldNumber::must(2), "B".to_owned());
            tk1.extend_with_key(FieldNumber::must(3), "C".to_owned());
            let mut tk2 = TupleKey::default();
            tk2.extend_with_key(FieldNumber::must(4), "D".to_owned());
            tk2.extend_with_key(FieldNumber::must(5), "E".to_owned());
            tk2.extend_with_key(FieldNumber::must(6), "F".to_owned());
            // preconditions
            assert_eq!(
                &[48, 65, 128, 80, 67, 0, 112, 67, 128],
                tk1.as_bytes()
            );
            assert_eq!(
                &[144, 69, 0, 176, 69, 128, 208, 71, 0],
                tk2.as_bytes()
            );
            // what we want to test
            tk1.append(&mut tk2);
            assert_eq!(
                &[
                    48, 65, 128, 80, 67, 0, 112, 67, 128, 144, 69, 0, 176, 69, 128, 208, 71, 0,
                ],
                tk1.as_bytes()
            );
            assert!(tk2.is_empty());
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
            tk1.extend_with_key(FieldNumber::must(1), "A".to_string());
            tk1.extend_with_key(FieldNumber::must(2), "B".to_string());
            tk1.extend_with_key(FieldNumber::must(3), "C".to_string());
            let mut iter = TupleKeyIterator::new(&tk1);

            let buf: &[u8] = &[48];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[65, 128];
            assert_eq!(Some(buf), iter.next());

            let buf: &[u8] = &[80];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[67, 0];
            assert_eq!(Some(buf), iter.next());

            let buf: &[u8] = &[112];
            assert_eq!(Some(buf), iter.next());
            let buf: &[u8] = &[67, 128];
            assert_eq!(Some(buf), iter.next());

            assert_eq!(None, iter.next());
        }
    }
}

#[cfg(test)]
mod elements {
    use super::*;

    fn test_helper<E: Element + Debug + Eq>(elem: E, exp: &[u8]) {
        let mut tk = TupleKey::default();
        elem.append_to(&mut tk);
        assert_eq!(exp, tk.as_bytes());
        let got = E::parse_from(exp).unwrap();
        assert_eq!(got, elem);
    }

    #[test]
    fn to_from_unit() {
        const VALUE: () = ();
        test_helper(
            VALUE,
            &[0],
        );
    }

    #[test]
    fn to_from_u32() {
        const VALUE: u32 = 0x1eaff00du32;
        test_helper(
            VALUE,
            &[0b00011111, 0b01010111, 0b11111101, 0b00000001, 0b11010000],
        );
    }

    #[test]
    fn to_from_u64() {
        const VALUE: u64 = 0x1eaff00d00c0ffeeu64;
        test_helper(
            VALUE,
            &[
                0b00011111, 0b01010111, 0b11111101, 0b00000001, 0b11010001, 0b00000111, 0b00000011,
                0b11111111, 0b11101111, 0b00000000,
            ],
        );
    }

    #[test]
    fn to_from_i32() {
        const VALUE: i32 = 0x1eaff00di32;
        test_helper(
            VALUE,
            &[0b10011111, 0b01010111, 0b11111101, 0b00000001, 0b11010000],
        );
    }

    #[test]
    fn to_from_i64() {
        const VALUE: i64 = 0x1eaff00d00c0ffeei64;
        test_helper(
            VALUE,
            &[
                0b10011111, 0b01010111, 0b11111101, 0b00000001, 0b11010001, 0b00000111, 0b00000011,
                0b11111111, 0b11101111, 0b00000000,
            ],
        );
    }

    #[test]
    fn to_from_vec_u8_empty() {
        test_helper(vec![], &[0]);
    }

    #[test]
    fn to_from_vec_u8() {
        test_helper(
            vec![0, 1, 2, 3],
            &[0b00000001, 0b00000001, 0b01000001, 0b01000001, 0b00110000],
        );
    }

    #[test]
    fn to_from_u8_16() {
        const VALUE: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        test_helper(
            VALUE,
            &[
                0b00000001, 0b00000001, 0b01000001, 0b01000001, 0b00110001, 0b00100001, 0b00010101,
                0b00001101, 0b00000111, 0b10000101, 0b00000011, 0b00100001, 0b10100001, 0b01011001,
                0b00110001, 0b00011011, 0b00001111, 0b00000111, 0b11000000,
            ],
        );
    }

    #[test]
    fn to_from_u8_32() {
        const VALUE: [u8; 32] = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ];
        test_helper(
            VALUE,
            &[
                0b00000001, 0b00000001, 0b01000001, 0b01000001, 0b00110001, 0b00100001, 0b00010101,
                0b00001101, 0b00000111, 0b10000101, 0b00000011, 0b00100001, 0b10100001, 0b01011001,
                0b00110001, 0b00011011, 0b00001111, 0b00000111, 0b11000101, 0b00000011, 0b00010001,
                0b10010001, 0b01001101, 0b00101001, 0b00010101, 0b10001011, 0b10000101, 0b11100011,
                0b10000001, 0b11001001, 0b01101001, 0b00110111, 0b00011101, 0b00001111, 0b01000111,
                0b11000011, 0b11110000,
            ],
        );
    }

    #[test]
    fn to_from_string_empty() {
        let value: String = "".to_owned();
        test_helper(value, &[0]);
    }

    #[test]
    fn to_from_string() {
        let value: String = "hello world".to_owned();
        test_helper(
            value,
            &[105, 51, 91, 141, 199, 121, 129, 239, 111, 185, 155, 141, 64],
        );
    }
}
