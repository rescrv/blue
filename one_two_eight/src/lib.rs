#![doc = include_str!("../README.md")]

use std::fmt::Write;
use std::fs::File;
use std::io::Read;

/// The number of bytes in a one_two_eight identifier.
pub const BYTES: usize = 16;

const SLICES: [(usize, usize); 5] = [(0, 4), (4, 6), (6, 8), (8, 10), (10, 16)];

/// Read a new ID from /dev/urandom
pub fn urandom() -> Option<[u8; BYTES]> {
    let mut f = match File::open("/dev/urandom") {
        Ok(f) => f,
        Err(_) => {
            return None;
        }
    };
    let mut id: [u8; BYTES] = [0u8; BYTES];
    let mut amt = 0;
    while amt < BYTES {
        let x = f.read(&mut id).ok()?;
        amt += x;
    }
    Some(id)
}

/// Encode 16B of random data in something aesthetically better, like UUID format.
pub fn encode(id: &[u8; BYTES]) -> String {
    let mut s = String::with_capacity(36);
    for &(start, limit) in SLICES.iter() {
        if start > 0 {
            s.push('-');
        }
        for c in &id[start..limit] {
            write!(&mut s, "{c:02x}").expect("unable to write to string");
        }
    }
    s
}

/// Turn the "aesthetically better" string back into bytes.
pub fn decode(s: &str) -> Option<[u8; BYTES]> {
    let mut result = [0u8; BYTES];
    let mut index = 0;
    let mut chars = s.chars();
    for &(start, limit) in SLICES.iter() {
        for _ in start..limit {
            let mut upper = chars.next()?;
            let mut lower = chars.next()?;
            if !upper.is_ascii_hexdigit() {
                return None;
            }
            if !lower.is_ascii_hexdigit() {
                return None;
            }
            upper.make_ascii_lowercase();
            lower.make_ascii_lowercase();
            const HEX: &str = "0123456789abcdef";
            let upper = HEX.find(upper).unwrap();
            let lower = HEX.find(lower).unwrap();

            result[index] = ((upper << 4) | lower) as u8;
            index += 1;
        }
        let dash = chars.next();
        if (limit < 16 && dash != Some('-')) || (limit == 16 && dash.is_some()) {
            return None;
        }
    }
    Some(result)
}

/// Generate a type with the given name and literal string prefix for human-readable types.
#[macro_export]
macro_rules! generate_id {
    ($what:ident, $prefix:literal) => {
        /// A one_two_eight identifier.
        #[derive(Eq, PartialEq, PartialOrd, Ord, Clone, Copy, Hash)]
        pub struct $what {
            /// The raw bytes for this identifier.
            pub id: [u8; $crate::BYTES],
        }

        impl $what {
            /// The smallest identifier.
            pub const BOTTOM: $what = $what {
                id: [0u8; $crate::BYTES],
            };

            /// The largest identifier.
            pub const TOP: $what = $what {
                id: [0xffu8; $crate::BYTES],
            };

            /// Generate a new identifier, failing if urandom fails.
            pub fn generate() -> Option<$what> {
                match $crate::urandom() {
                    Some(id) => Some($what { id }),
                    None => None,
                }
            }

            /// Construct an identifier from its human-readable form.
            pub fn from_human_readable(s: &str) -> Option<Self> {
                let prefix = $prefix;
                if !s.starts_with(prefix) {
                    return None;
                }
                match $crate::decode(&s[prefix.len()..]) {
                    Some(x) => Some(Self::new(x)),
                    None => None,
                }
            }

            /// Construct a string corresponding to the human-readable form of this identifier.
            pub fn human_readable(&self) -> String {
                let readable = $prefix.to_string();
                readable + &$crate::encode(&self.id)
            }

            /// Return the prefix-free encoding of this identifier.
            pub fn prefix_free_readable(&self) -> String {
                $crate::encode(&self.id)
            }

            /// Create a new identifier from raw bytes.
            pub fn new(id: [u8; $crate::BYTES]) -> Self {
                Self { id }
            }

            /// Increment the identifier by one.
            pub fn next(mut self) -> Self {
                for byte_index in (0..$crate::BYTES).rev() {
                    self.id[byte_index] = self.id[byte_index].wrapping_add(1);
                    if self.id[byte_index] != 0 {
                        break;
                    }
                }
                self
            }
        }

        impl Default for $what {
            fn default() -> $what {
                $what::BOTTOM
            }
        }

        impl std::fmt::Debug for $what {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}{}", $prefix, $crate::encode(&self.id))
            }
        }

        impl std::fmt::Display for $what {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}{}", $prefix, $crate::encode(&self.id))
            }
        }

        impl std::str::FromStr for $what {
            type Err = &'static str;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match $what::from_human_readable(s) {
                    Some(x) => Ok(x),
                    None => Err("invalid human-readable identifier"),
                }
            }
        }
    };
}

/// Implement protocol buffers for the id.
#[cfg(feature = "generate_id_prototk")]
#[macro_export]
macro_rules! generate_id_prototk {
    ($what:ident) => {
        impl buffertk::Packable for $what {
            fn pack_sz(&self) -> usize {
                let id_buf: &[u8] = &self.id;
                buffertk::stack_pack(prototk::tag!(1, LengthDelimited))
                    .pack(id_buf)
                    .pack_sz()
            }

            fn pack(&self, buf: &mut [u8]) {
                let id_buf: &[u8] = &self.id;
                buffertk::stack_pack(prototk::tag!(1, LengthDelimited))
                    .pack(id_buf)
                    .into_slice(buf);
            }
        }

        impl<'a> buffertk::Unpackable<'a> for $what {
            type Error = prototk::Error;

            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), prototk::Error> {
                let mut up = buffertk::Unpacker::new(buf);
                let tag: buffertk::v64 = up.unpack()?;
                let v: buffertk::v64 = up.unpack()?;
                let v: usize = v.into();
                let rem = up.remain();
                if rem.len() < v {
                    return Err(prototk::Error::BufferTooShort {
                        required: v,
                        had: rem.len(),
                    });
                }
                // TODO(rescrv): Have an error if v != 16.
                let id = $what {
                    id: rem[..16].try_into().unwrap(),
                };
                Ok((id, &rem[v..]))
            }
        }

        impl<'a> prototk::Message<'a> for $what {}

        impl<'a> prototk::FieldPackHelper<'a, ::prototk::field_types::message<$what>> for $what {
            fn field_pack_sz(&self, tag: &::prototk::Tag) -> usize {
                use buffertk::{stack_pack, Packable};
                use prototk::{FieldPackHelper, FieldType, Message};
                stack_pack(tag)
                    .pack(stack_pack(self).length_prefixed())
                    .pack_sz()
            }

            fn field_pack(&self, tag: &::prototk::Tag, out: &mut [u8]) {
                use buffertk::{stack_pack, Packable};
                use prototk::{FieldPackHelper, FieldType, Message};
                stack_pack(tag)
                    .pack(stack_pack(self).length_prefixed())
                    .into_slice(out);
            }
        }

        impl<'a> ::prototk::FieldUnpackHelper<'a, ::prototk::field_types::message<$what>>
            for $what
        {
            fn merge_field(&mut self, proto: ::prototk::field_types::message<$what>) {
                *self = proto.unwrap_message();
            }
        }

        impl From<::prototk::field_types::message<$what>> for $what {
            fn from(proto: ::prototk::field_types::message<$what>) -> Self {
                proto.unwrap_message()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urandom_is_nonzero() {
        assert_ne!(Some([0u8; BYTES]), urandom());
    }

    #[test]
    fn id_bytes_is_sixteen() {
        assert_eq!(BYTES, 16);
    }

    #[test]
    fn encode_id() {
        let id = [0x55u8; BYTES];
        assert_eq!(encode(&id), "55555555-5555-5555-5555-555555555555");
    }

    #[test]
    fn decode_id() {
        let id = [0x55u8; BYTES];
        assert_eq!(decode("55555555-5555-5555-5555-555555555555"), Some(id));
    }

    generate_id!(FooID, "foo:");

    #[test]
    fn generate_id() {
        let _x = FooID::generate().unwrap();
        let id = FooID::new([0xffu8; BYTES]);
        assert_eq!(
            "foo:ffffffff-ffff-ffff-ffff-ffffffffffff",
            id.human_readable()
        );
        assert_eq!(
            "ffffffff-ffff-ffff-ffff-ffffffffffff",
            id.prefix_free_readable()
        );
        assert_eq!(
            Some(id),
            FooID::from_human_readable("foo:ffffffff-ffff-ffff-ffff-ffffffffffff")
        );
        assert_eq!([0x00u8; BYTES], FooID::BOTTOM.id);
        assert_eq!([0xffu8; BYTES], FooID::TOP.id);
    }

    #[test]
    fn next() {
        assert_eq!(FooID::BOTTOM, FooID::TOP.next());
        let id = FooID {
            id: [0x55u8; BYTES],
        };
        assert_eq!(
            "foo:55555555-5555-5555-5555-555555555555",
            id.human_readable()
        );
        assert_eq!(
            "foo:55555555-5555-5555-5555-555555555556",
            id.next().human_readable()
        );
        let id = FooID {
            id: [
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xfe,
            ],
        };
        assert_eq!(
            "foo:ffffffff-ffff-ffff-ffff-fffffffffffe",
            id.human_readable()
        );
        assert_eq!(
            "foo:ffffffff-ffff-ffff-ffff-ffffffffffff",
            id.next().human_readable()
        );
    }
}
