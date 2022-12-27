//! This module provides an implementation of the variable integer encoding specified in the
//! [protobuf encoding documentation](https://developers.google.com/protocol-buffers/docs/encoding).
//!
//! By convention the From<I> and Into<I> traits are implemented for all common integer types I.
//! They will silently truncate and it is up to higher level code to unpack to full v64 and check
//! for overflow when casting.

use super::Error;
use super::Packable;
use super::Unpackable;

use std::convert::TryInto;

////////////////////////////////////////////// Varint //////////////////////////////////////////////

/// v64 is the type of a variable integer encoding.  It can represent any value of 64-bits or
/// fewer.  The encoding follows the protocol buffer spec, which means that negative numbers will
/// always serialize to ten bytes.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct v64 {
    x: u64,
}

impl From<u8> for v64 {
    fn from(x: u8) -> v64 {
        v64 { x: x as u64 }
    }
}

impl TryInto<u8> for v64 {
    type Error = Error;

    fn try_into(self) -> Result<u8, Error> {
        match self.x.try_into() {
            Ok(x) => { Ok(x) },
            Err(_) => { Err(Error::UnsignedOverflow{ value: self.x }) },
        }
    }
}

impl From<u16> for v64 {
    fn from(x: u16) -> v64 {
        v64 { x: x as u64 }
    }
}

impl TryInto<u16> for v64 {
    type Error = Error;

    fn try_into(self) -> Result<u16, Error> {
        match self.x.try_into() {
            Ok(x) => { Ok(x) },
            Err(_) => { Err(Error::UnsignedOverflow{ value: self.x }) },
        }
    }
}

impl From<u32> for v64 {
    fn from(x: u32) -> v64 {
        v64 { x: x as u64 }
    }
}

impl TryInto<u32> for v64 {
    type Error = Error;

    fn try_into(self) -> Result<u32, Error> {
        match self.x.try_into() {
            Ok(x) => { Ok(x) },
            Err(_) => { Err(Error::UnsignedOverflow{ value: self.x }) },
        }
    }
}

impl From<u64> for v64 {
    fn from(x: u64) -> v64 {
        v64 { x }
    }
}

impl Into<u64> for v64 {
    fn into(self) -> u64 {
        self.x
    }
}

impl From<i8> for v64 {
    fn from(x: i8) -> v64 {
        v64 { x: x as u64 }
    }
}

impl TryInto<i8> for v64 {
    type Error = Error;

    fn try_into(self) -> Result<i8, Error> {
        let value: i64 = self.x as i64;
        match value.try_into() {
            Ok(x) => { Ok(x) },
            Err(_) => { Err(Error::SignedOverflow{ value }) },
        }
    }
}

impl From<i16> for v64 {
    fn from(x: i16) -> v64 {
        v64 { x: x as u64 }
    }
}

impl TryInto<i16> for v64 {
    type Error = Error;

    fn try_into(self) -> Result<i16, Error> {
        let value: i64 = self.x as i64;
        match value.try_into() {
            Ok(x) => { Ok(x) },
            Err(_) => { Err(Error::SignedOverflow{ value }) },
        }
    }
}

impl From<i32> for v64 {
    fn from(x: i32) -> v64 {
        v64 { x: x as u64 }
    }
}

impl TryInto<i32> for v64 {
    type Error = Error;

    fn try_into(self) -> Result<i32, Error> {
        let value: i64 = self.x as i64;
        match value.try_into() {
            Ok(x) => { Ok(x) },
            Err(_) => { Err(Error::SignedOverflow{ value }) },
        }
    }
}

impl From<i64> for v64 {
    fn from(x: i64) -> v64 {
        v64 { x: x as u64 }
    }
}

impl Into<i64> for v64 {
    fn into(self) -> i64 {
        self.x as i64
    }
}

impl From<usize> for v64 {
    fn from(x: usize) -> v64 {
        // unwrap because we assume and test this is safe
        let x: u64 = x.try_into().unwrap();
        v64 { x: x }
    }
}

impl Into<usize> for v64 {
    fn into(self) -> usize {
        // unwrap because we assume and test this is safe
        let x: usize = self.x.try_into().unwrap();
        x
    }
}

impl Packable for v64 {
    fn pack_sz(&self) -> usize {
        let mut x: u64 = self.x;
        let mut count: usize = 1;
        x >>= 7;
        while x > 0 {
            x >>= 7;
            count += 1;
        }
        count
    }

    fn pack(&self, out: &mut [u8]) {
        let mut x: u64 = self.x;
        out[0] = (x & 0x7f) as u8;
        x >>= 7;
        let mut idx: usize = 1;
        while x > 0 {
            out[idx - 1] |= 128;
            out[idx] = (x & 0x7f) as u8;
            idx += 1;
            x >>= 7;
        }
    }
}

impl<'a> Unpackable<'a> for v64 {
    fn unpack<'b>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error>
        where
            'b: 'a,
    {
        assert!(buf.len() > 0);
        let bytes: usize = if buf.len() < 10 { buf.len() } else { 10 };
        let mut ret = 0u64;
        let mut idx = 0;
        let mut shl = 0;
        while idx + 1 < bytes && buf[idx] & 128 != 0 {
            ret |= (buf[idx] as u64 & 127) << shl;
            idx += 1;
            shl += 7;
        }
        if buf[idx] & 128 == 0 {
            ret |= (buf[idx] as u64 & 127) << shl;
            idx += 1;
            let ret: v64 = ret.into();
            Ok((ret, &buf[idx..]))
        } else {
            Err(Error::VarintOverflow { bytes })
        }
    }
}

///////////////////////////////////////////// mod tests ////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn from_into_x<X, E>(x: X)
        where
            v64: std::convert::From<X> + std::convert::TryInto<X, Error=E>,
            X: std::fmt::Debug + PartialEq + Copy,
            E: std::fmt::Debug,

    {
        let v: v64 = v64::from(x);
        let x2: X = v.try_into().unwrap();
        assert_eq!(x, x2, "value did not survive a .into().into()");
    }

    #[test]
    fn from_into_u8() {
        from_into_x(u8::min_value());
        from_into_x(u8::max_value());
        from_into_x(1u8);
    }

    #[test]
    fn try_into_u8() {
        let x: u64 = (u8::max_value() as u64) + 1;
        let v: v64 = v64::from(x);
        let x2: Result<u8, Error> = v.try_into();
        assert_eq!(Err(Error::UnsignedOverflow{ value: x }), x2);
    }

    #[test]
    fn from_into_u16() {
        from_into_x(u16::min_value());
        from_into_x(u16::max_value());
        from_into_x(1u16);
    }

    #[test]
    fn try_into_u16() {
        let x: u64 = (u16::max_value() as u64) + 1;
        let v: v64 = v64::from(x);
        let x2: Result<u16, Error> = v.try_into();
        assert_eq!(Err(Error::UnsignedOverflow{ value: x }), x2);
    }

    #[test]
    fn from_into_u32() {
        from_into_x(u32::min_value());
        from_into_x(u32::max_value());
        from_into_x(1u32);
    }

    #[test]
    fn try_into_u32() {
        let x: u64 = (u32::max_value() as u64) + 1;
        let v: v64 = v64::from(x);
        let x2: Result<u32, Error> = v.try_into();
        assert_eq!(Err(Error::UnsignedOverflow{ value: x }), x2);
    }

    #[test]
    fn from_into_u64() {
        from_into_x(u64::min_value());
        from_into_x(u64::max_value());
        from_into_x(1u64);
    }

    #[test]
    fn from_into_i8() {
        from_into_x(i8::min_value());
        from_into_x(i8::max_value());
        from_into_x(-1i8);
        from_into_x(0i8);
        from_into_x(1i8);
    }

    #[test]
    fn try_into_i8() {
        let x: i64 = (i8::max_value() as i64) + 1;
        let v: v64 = v64::from(x);
        let x2: Result<i8, Error> = v.try_into();
        assert_eq!(Err(Error::SignedOverflow{ value: x }), x2);
    }

    #[test]
    fn from_into_i16() {
        from_into_x(i16::min_value());
        from_into_x(i16::max_value());
        from_into_x(-1i16);
        from_into_x(0i16);
        from_into_x(1i16);
    }

    #[test]
    fn try_into_i16() {
        let x: i64 = (i16::max_value() as i64) + 1;
        let v: v64 = v64::from(x);
        let x2: Result<i16, Error> = v.try_into();
        assert_eq!(Err(Error::SignedOverflow{ value: x }), x2);
    }

    #[test]
    fn from_into_i32() {
        from_into_x(i32::min_value());
        from_into_x(i32::max_value());
        from_into_x(-1i32);
        from_into_x(0i32);
        from_into_x(1i32);
    }

    #[test]
    fn try_into_i32() {
        let x: i64 = (i32::max_value() as i64) + 1;
        let v: v64 = v64::from(x);
        let x2: Result<i32, Error> = v.try_into();
        assert_eq!(Err(Error::SignedOverflow{ value: x }), x2);
    }

    #[test]
    fn from_into_i64() {
        from_into_x(i64::min_value());
        from_into_x(i64::max_value());
        from_into_x(-1i64);
        from_into_x(0i64);
        from_into_x(1i64);
    }

    #[test]
    fn from_into_usize() {
        from_into_x(usize::min_value());
        from_into_x(usize::max_value());
        from_into_x(1usize);
    }

    #[test]
    fn assumption_u64_holds_usize() {
        let min: u64 = usize::min_value().try_into().unwrap();
        let min: usize = min.try_into().unwrap();
        assert_eq!(usize::min_value(), min, "u64 cannot hold usize::min_value()");
        let max: u64 = usize::max_value().try_into().unwrap();
        let max: usize = max.try_into().unwrap();
        assert_eq!(usize::max_value(), max, "u64 cannot hold usize::max_value()");
    }

    const TESTS: &[(u64, usize, &[u8])] = &[
        (0, 1, &[0]),
        (1, 1, &[1]),
        ((1 << 7) - 1, 1, &[127]),
        ((1 << 7), 2, &[128, 1]),
        ((1 << 14) - 1, 2, &[255, 127]),
        ((1 << 14), 3, &[128, 128, 1]),
        ((1 << 21) - 1, 3, &[255, 255, 127]),
        ((1 << 21), 4, &[128, 128, 128, 1]),
        ((1 << 28) - 1, 4, &[255, 255, 255, 127]),
        ((1 << 28), 5, &[128, 128, 128, 128, 1]),
        ((1 << 35) - 1, 5, &[255, 255, 255, 255, 127]),
        ((1 << 35), 6, &[128, 128, 128, 128, 128, 1]),
        ((1 << 42) - 1, 6, &[255, 255, 255, 255, 255, 127]),
        ((1 << 42), 7, &[128, 128, 128, 128, 128, 128, 1]),
        ((1 << 49) - 1, 7, &[255, 255, 255, 255, 255, 255, 127]),
        ((1 << 49), 8, &[128, 128, 128, 128, 128, 128, 128, 1]),
        ((1 << 56) - 1, 8, &[255, 255, 255, 255, 255, 255, 255, 127]),
        ((1 << 56), 9, &[128, 128, 128, 128, 128, 128, 128, 128, 1]),
        (
            (1 << 63) - 1,
            9,
            &[255, 255, 255, 255, 255, 255, 255, 255, 127],
        ),
        (
            (1 << 63),
            10,
            &[128, 128, 128, 128, 128, 128, 128, 128, 128, 1],
        ),
    ];

    #[test]
    fn pack_varint() {
        for (idx, &(num, bytes, enc)) in TESTS.iter().enumerate() {
            println!("test case={} x={}, |x|={}, s(x)={:?}", idx, num, bytes, enc);
            let mut buf: [u8; 10] = [0; 10];
            assert_eq!(bytes, enc.len(), "human got test case wrong?");
            assert!(bytes <= buf.len(), "human made buffer too small?");
            let num: v64 = num.into();
            let req = num.pack_sz();
            assert_eq!(bytes, req, "human got pack_sz wrong?");
            num.pack(&mut buf[..bytes]);
            assert_eq!(enc, &buf[..bytes], "human got encoder wrong?");
        }
    }

    #[test]
    fn unpack_varint() {
        for (idx, &(num, bytes, enc)) in TESTS.iter().enumerate() {
            println!("test case={} x={}, |x|={}, s(x)={:?}", idx, num, bytes, enc);
            assert_eq!(bytes, enc.len(), "human got test case wrong?");
            assert!(enc.len() <= 10, "human got test harness wrong?");
            let mut buf: [u8; 10] = [0xff; 10];
            for i in 0..enc.len() {
                buf[i] = enc[i];
            }
            let (x, rem): (v64, &[u8]) = Unpackable::unpack(&buf).unwrap();
            let v: v64 = num.into();
            assert_eq!(v, x, "human got decode wrong?");
            assert_eq!(rem, &buf[bytes..], "human got remainder wrong?");
        }
    }

    // TODO(rescrv): test unhappy paths
}
