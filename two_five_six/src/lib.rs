#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

use std::fs::File;
use std::io::Read;

/// The number of bytes in a two_five_six identifier.
pub const BYTES: usize = 32;

/// The length of the encoded string (43 characters, no padding).
pub const ENCODED_LENGTH: usize = 43;

/// URL-safe Base64 alphabet (RFC 4648).
const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Decode table for URL-safe Base64. Invalid characters map to 0xFF.
const BASE64_DECODE: [u8; 128] = {
    let mut table = [0xFFu8; 128];
    let mut i = 0usize;
    while i < 64 {
        table[BASE64_ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    table
};

/// Error type for TwoFiveSix operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The label contains invalid Base64 characters.
    InvalidLabelCharacter(char),
    /// The encoded string has an invalid length.
    InvalidLength(usize),
    /// The encoded string contains invalid Base64 characters.
    InvalidBase64Character(char),
    /// Failed to read from urandom.
    UrandomFailure,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidLabelCharacter(c) => write!(f, "invalid label character: {c:?}"),
            Error::InvalidLength(len) => {
                write!(f, "invalid length: expected {ENCODED_LENGTH}, got {len}")
            }
            Error::InvalidBase64Character(c) => write!(f, "invalid Base64 character: {c:?}"),
            Error::UrandomFailure => write!(f, "failed to read from /dev/urandom"),
        }
    }
}

impl std::error::Error for Error {}

/// Check if a character is a valid URL-safe Base64 character.
fn is_valid_base64_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Check if a character is alphanumeric (Base62, excluding `-` and `_`).
fn is_alphanumeric(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// Encode 32 bytes to a 43-character URL-safe Base64 string (no padding).
fn encode(bytes: &[u8; BYTES]) -> String {
    let mut result = String::with_capacity(ENCODED_LENGTH);

    // Process 3 bytes at a time to produce 4 Base64 characters.
    let mut i = 0;
    while i + 3 <= BYTES {
        let b0 = bytes[i] as usize;
        let b1 = bytes[i + 1] as usize;
        let b2 = bytes[i + 2] as usize;

        result.push(BASE64_ALPHABET[b0 >> 2] as char);
        result.push(BASE64_ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        result.push(BASE64_ALPHABET[((b1 & 0x0F) << 2) | (b2 >> 6)] as char);
        result.push(BASE64_ALPHABET[b2 & 0x3F] as char);

        i += 3;
    }

    // Handle remaining 2 bytes (32 = 30 + 2, so we have 2 bytes left).
    // 2 bytes = 16 bits, which encodes to 3 Base64 characters (18 bits, 2 padding bits).
    if i + 2 == BYTES {
        let b0 = bytes[i] as usize;
        let b1 = bytes[i + 1] as usize;

        result.push(BASE64_ALPHABET[b0 >> 2] as char);
        result.push(BASE64_ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        result.push(BASE64_ALPHABET[(b1 & 0x0F) << 2] as char);
    }

    result
}

/// Decode a 43-character URL-safe Base64 string to 32 bytes.
fn decode(s: &str) -> Result<[u8; BYTES], Error> {
    if s.len() != ENCODED_LENGTH {
        return Err(Error::InvalidLength(s.len()));
    }

    let chars: Vec<char> = s.chars().collect();
    let mut result = [0u8; BYTES];

    // Decode 4 characters at a time to produce 3 bytes.
    let mut byte_idx = 0;
    let mut char_idx = 0;

    while char_idx + 4 <= ENCODED_LENGTH {
        let mut values = [0u8; 4];
        for (j, v) in values.iter_mut().enumerate() {
            let c = chars[char_idx + j];
            if !c.is_ascii() || c as usize >= 128 {
                return Err(Error::InvalidBase64Character(c));
            }
            let val = BASE64_DECODE[c as usize];
            if val == 0xFF {
                return Err(Error::InvalidBase64Character(c));
            }
            *v = val;
        }

        result[byte_idx] = (values[0] << 2) | (values[1] >> 4);
        result[byte_idx + 1] = (values[1] << 4) | (values[2] >> 2);
        result[byte_idx + 2] = (values[2] << 6) | values[3];

        byte_idx += 3;
        char_idx += 4;
    }

    // Handle remaining 3 characters (produces 2 bytes).
    // 43 = 40 + 3, so we have 3 characters left.
    if char_idx + 3 == ENCODED_LENGTH {
        let mut values = [0u8; 3];
        for (j, v) in values.iter_mut().enumerate() {
            let c = chars[char_idx + j];
            if !c.is_ascii() || c as usize >= 128 {
                return Err(Error::InvalidBase64Character(c));
            }
            let val = BASE64_DECODE[c as usize];
            if val == 0xFF {
                return Err(Error::InvalidBase64Character(c));
            }
            *v = val;
        }

        result[byte_idx] = (values[0] << 2) | (values[1] >> 4);
        result[byte_idx + 1] = (values[1] << 4) | (values[2] >> 2);
    }

    Ok(result)
}

/// Read 32 bytes from /dev/urandom.
fn urandom() -> Result<[u8; BYTES], Error> {
    let mut f = File::open("/dev/urandom").map_err(|_| Error::UrandomFailure)?;
    let mut id = [0u8; BYTES];
    let mut amt = 0;
    while amt < BYTES {
        let x = f.read(&mut id[amt..]).map_err(|_| Error::UrandomFailure)?;
        if x == 0 {
            return Err(Error::UrandomFailure);
        }
        amt += x;
    }
    Ok(id)
}

/// A 256-bit identifier represented as a 43-character URL-safe Base64 string.
#[derive(Eq, PartialEq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct TwoFiveSix {
    id: [u8; BYTES],
}

impl TwoFiveSix {
    /// The smallest identifier (all zeros).
    pub const BOTTOM: TwoFiveSix = TwoFiveSix { id: [0u8; BYTES] };

    /// The largest identifier (all 0xFF).
    pub const TOP: TwoFiveSix = TwoFiveSix {
        id: [0xFFu8; BYTES],
    };

    /// Create a new identifier from raw bytes.
    pub fn new(id: [u8; BYTES]) -> Self {
        Self { id }
    }

    /// Return the raw bytes of this identifier.
    pub fn as_bytes(&self) -> &[u8; BYTES] {
        &self.id
    }

    /// Generate a new random identifier without a label.
    pub fn generate() -> Result<Self, Error> {
        Self::generate_with_label(None, '_')
    }

    /// Generate a new identifier with an optional label and separator.
    ///
    /// The label must consist only of valid Base64 characters (`A-Za-z0-9-_`).
    /// The separator must be either `-` or `_`.
    ///
    /// The algorithm uses rejection sampling to ensure the suffix (after the label
    /// and separator) contains only alphanumeric characters (no `-` or `_`).
    pub fn generate_with_label(label: Option<&str>, separator: char) -> Result<Self, Error> {
        // Validate label characters.
        let prefix = if let Some(label) = label {
            for c in label.chars() {
                if !is_valid_base64_char(c) {
                    return Err(Error::InvalidLabelCharacter(c));
                }
            }
            format!("{label}{separator}")
        } else {
            String::new()
        };

        let prefix_len = prefix.len();

        // Rejection sampling loop.
        loop {
            let bytes = urandom()?;
            let encoded = encode(&bytes);

            // Check that the suffix (after prefix position) contains no `-` or `_`.
            let suffix = &encoded[prefix_len..];
            if suffix.chars().all(is_alphanumeric) {
                // Overwrite the prefix.
                let final_str = if prefix_len > 0 {
                    format!("{prefix}{suffix}")
                } else {
                    encoded
                };

                // Decode back to bytes.
                return Ok(Self {
                    id: decode(&final_str)?,
                });
            }
        }
    }

    /// Encode this identifier to its 43-character URL-safe Base64 representation.
    pub fn encode(&self) -> String {
        encode(&self.id)
    }

    /// Decode a 43-character URL-safe Base64 string into an identifier.
    pub fn decode(s: &str) -> Result<Self, Error> {
        Ok(Self { id: decode(s)? })
    }

    /// Parse the label from an identifier string.
    ///
    /// Returns `(label, suffix)` by splitting on the last occurrence of `-` or `_`.
    /// If no separator is found, returns `(None, full_string)`.
    pub fn parse_label(s: &str) -> (Option<&str>, &str) {
        // Find the last occurrence of `-` or `_`.
        let last_sep = s.rfind(['-', '_']);
        match last_sep {
            Some(idx) => (Some(&s[..idx]), &s[idx + 1..]),
            None => (None, s),
        }
    }

    /// Extract the label from this identifier.
    ///
    /// Returns `Some(label)` if a separator is found, `None` otherwise.
    pub fn label(&self) -> Option<String> {
        let encoded = self.encode();
        Self::parse_label(&encoded).0.map(|s| s.to_string())
    }

    /// Increment the identifier by one.
    pub fn next(mut self) -> Self {
        for byte_index in (0..BYTES).rev() {
            self.id[byte_index] = self.id[byte_index].wrapping_add(1);
            if self.id[byte_index] != 0 {
                break;
            }
        }
        self
    }
}

impl Default for TwoFiveSix {
    fn default() -> Self {
        Self::BOTTOM
    }
}

impl std::fmt::Debug for TwoFiveSix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", encode(&self.id))
    }
}

impl std::fmt::Display for TwoFiveSix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", encode(&self.id))
    }
}

impl std::str::FromStr for TwoFiveSix {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::decode(s)
    }
}

/// Generate a type with the given name for labeled TwoFiveSix identifiers.
#[macro_export]
macro_rules! generate_id {
    ($what:ident, $label:literal, $separator:literal) => {
        /// A two_five_six identifier with a fixed label.
        #[derive(Eq, PartialEq, PartialOrd, Ord, Clone, Copy, Hash)]
        pub struct $what {
            id: $crate::TwoFiveSix,
        }

        impl $what {
            #![allow(unused)]

            /// The label for this identifier type.
            pub const LABEL: &'static str = $label;

            /// The separator character.
            pub const SEPARATOR: char = $separator;

            /// Generate a new identifier with the configured label.
            pub fn generate() -> Result<Self, $crate::Error> {
                Ok(Self {
                    id: $crate::TwoFiveSix::generate_with_label(Some($label), $separator)?,
                })
            }

            /// Create from raw bytes.
            pub fn new(id: [u8; $crate::BYTES]) -> Self {
                Self {
                    id: $crate::TwoFiveSix::new(id),
                }
            }

            /// Return the raw bytes.
            pub fn as_bytes(&self) -> &[u8; $crate::BYTES] {
                self.id.as_bytes()
            }

            /// Encode to string.
            pub fn encode(&self) -> String {
                self.id.encode()
            }

            /// Decode from string, validating the label prefix.
            pub fn decode(s: &str) -> Result<Self, $crate::Error> {
                let id = $crate::TwoFiveSix::decode(s)?;
                Ok(Self { id })
            }

            /// Increment the identifier by one.
            pub fn next(self) -> Self {
                Self { id: self.id.next() }
            }
        }

        impl Default for $what {
            fn default() -> Self {
                Self {
                    id: $crate::TwoFiveSix::BOTTOM,
                }
            }
        }

        impl std::fmt::Debug for $what {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.id)
            }
        }

        impl std::fmt::Display for $what {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.id)
            }
        }

        impl std::str::FromStr for $what {
            type Err = $crate::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::decode(s)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_is_32() {
        assert_eq!(BYTES, 32);
    }

    #[test]
    fn encoded_length_is_43() {
        assert_eq!(ENCODED_LENGTH, 43);
    }

    #[test]
    fn encode_zeros() {
        let bytes = [0u8; BYTES];
        let encoded = encode(&bytes);
        assert_eq!(encoded.len(), ENCODED_LENGTH);
        assert_eq!(encoded, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        println!("encode_zeros: {encoded}");
    }

    #[test]
    fn encode_ones() {
        let bytes = [0xFFu8; BYTES];
        let encoded = encode(&bytes);
        assert_eq!(encoded.len(), ENCODED_LENGTH);
        assert_eq!(encoded, "__________________________________________8");
        println!("encode_ones: {encoded}");
    }

    #[test]
    fn decode_zeros() {
        let decoded = decode("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
        assert_eq!(decoded, [0u8; BYTES]);
        println!("decode_zeros: {:?}", decoded);
    }

    #[test]
    fn decode_ones() {
        let decoded = decode("__________________________________________8").unwrap();
        assert_eq!(decoded, [0xFFu8; BYTES]);
        println!("decode_ones: {:?}", decoded);
    }

    #[test]
    fn roundtrip_encoding() {
        for i in 0..=255u8 {
            let mut bytes = [i; BYTES];
            bytes[0] = i;
            bytes[31] = 255 - i;
            let encoded = encode(&bytes);
            assert_eq!(encoded.len(), ENCODED_LENGTH);
            let decoded = decode(&encoded).unwrap();
            assert_eq!(bytes, decoded, "roundtrip failed for pattern {i}");
        }
        println!("roundtrip_encoding: all 256 patterns passed");
    }

    #[test]
    fn invalid_length() {
        let err = decode("AAAA").unwrap_err();
        assert_eq!(err, Error::InvalidLength(4));
        println!("invalid_length: {:?}", err);
    }

    #[test]
    fn invalid_character() {
        let err = decode("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA!").unwrap_err();
        assert_eq!(err, Error::InvalidBase64Character('!'));
        println!("invalid_character: {:?}", err);
    }

    #[test]
    fn generate_without_label() {
        let id = TwoFiveSix::generate().unwrap();
        let encoded = id.encode();
        assert_eq!(encoded.len(), ENCODED_LENGTH);
        // All characters should be alphanumeric (no label means full rejection sampling).
        assert!(
            encoded.chars().all(is_alphanumeric),
            "generated ID without label should be all alphanumeric: {encoded}"
        );
        println!("generate_without_label: {encoded}");
    }

    #[test]
    fn generate_with_label() {
        let id = TwoFiveSix::generate_with_label(Some("user"), '_').unwrap();
        let encoded = id.encode();
        assert_eq!(encoded.len(), ENCODED_LENGTH);
        assert!(
            encoded.starts_with("user_"),
            "expected prefix 'user_', got: {encoded}"
        );
        // Suffix should be alphanumeric.
        let suffix = &encoded[5..];
        assert!(
            suffix.chars().all(is_alphanumeric),
            "suffix should be alphanumeric: {suffix}"
        );
        println!("generate_with_label: {encoded}");
    }

    #[test]
    fn generate_with_hyphen_in_label() {
        let id = TwoFiveSix::generate_with_label(Some("my-app"), '_').unwrap();
        let encoded = id.encode();
        assert_eq!(encoded.len(), ENCODED_LENGTH);
        assert!(
            encoded.starts_with("my-app_"),
            "expected prefix 'my-app_', got: {encoded}"
        );
        println!("generate_with_hyphen_in_label: {encoded}");
    }

    #[test]
    fn parse_label_simple() {
        let (label, suffix) = TwoFiveSix::parse_label("user_3b6HqZ0gYtFdRsA9c4x2uE0M1n2O3P4Q5R6S");
        assert_eq!(label, Some("user"));
        assert_eq!(suffix, "3b6HqZ0gYtFdRsA9c4x2uE0M1n2O3P4Q5R6S");
        println!("parse_label_simple: label={:?}, suffix={}", label, suffix);
    }

    #[test]
    fn parse_label_with_hyphen() {
        let (label, suffix) = TwoFiveSix::parse_label("my-app_3b6HqZ0gYtFdRsA9c4x2uE0M1n2O3P4Q5");
        assert_eq!(label, Some("my-app"));
        assert_eq!(suffix, "3b6HqZ0gYtFdRsA9c4x2uE0M1n2O3P4Q5");
        println!(
            "parse_label_with_hyphen: label={:?}, suffix={}",
            label, suffix
        );
    }

    #[test]
    fn parse_label_no_separator() {
        let (label, suffix) =
            TwoFiveSix::parse_label("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        assert_eq!(label, None);
        assert_eq!(suffix, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        println!(
            "parse_label_no_separator: label={:?}, suffix={}",
            label, suffix
        );
    }

    #[test]
    fn invalid_label_character() {
        let err = TwoFiveSix::generate_with_label(Some("user!"), '_').unwrap_err();
        assert_eq!(err, Error::InvalidLabelCharacter('!'));
        println!("invalid_label_character: {:?}", err);
    }

    #[test]
    fn decode_is_inverse_of_encode() {
        for _ in 0..10 {
            let id = TwoFiveSix::generate().unwrap();
            let encoded = id.encode();
            let decoded = TwoFiveSix::decode(&encoded).unwrap();
            assert_eq!(id, decoded);
        }
        println!("decode_is_inverse_of_encode: 10 iterations passed");
    }

    #[test]
    fn next_increments() {
        let id = TwoFiveSix::BOTTOM;
        let next = id.next();
        assert_eq!(next.as_bytes()[31], 1);
        assert_eq!(&next.as_bytes()[..31], &[0u8; 31]);
        println!("next_increments: {:?} -> {:?}", id, next);
    }

    #[test]
    fn next_wraps() {
        let id = TwoFiveSix::TOP;
        let next = id.next();
        assert_eq!(next, TwoFiveSix::BOTTOM);
        println!("next_wraps: {:?} -> {:?}", id, next);
    }

    generate_id!(UserId, "user", '_');

    #[test]
    fn generated_type_works() {
        let id = UserId::generate().unwrap();
        let encoded = id.encode();
        assert_eq!(encoded.len(), ENCODED_LENGTH);
        assert!(
            encoded.starts_with("user_"),
            "expected prefix 'user_', got: {encoded}"
        );
        println!("generated_type_works: {encoded}");
    }

    #[test]
    fn test_is_valid_base64_char() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_".chars() {
            assert!(is_valid_base64_char(c));
        }
        for c in "!@#$%^&*()+=".chars() {
            assert!(!is_valid_base64_char(c));
        }
    }

    #[test]
    fn test_is_alphanumeric() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".chars() {
            assert!(is_alphanumeric(c));
        }
        for c in "-_!@#$%^&*()+=".chars() {
            assert!(!is_alphanumeric(c));
        }
    }

    #[test]
    fn roundtrip_random() {
        for _ in 0..100 {
            let bytes = urandom().unwrap();
            let encoded = encode(&bytes);
            let decoded = decode(&encoded).unwrap();
            assert_eq!(bytes, decoded);
        }
    }

    #[test]
    fn generate_with_empty_label() {
        let id = TwoFiveSix::generate_with_label(Some(""), '_').unwrap();
        let encoded = id.encode();
        assert!(encoded.starts_with('_'));
        let suffix = &encoded[1..];
        assert!(suffix.chars().all(is_alphanumeric));
    }

    #[test]
    fn generate_with_label_containing_separator() {
        let id = TwoFiveSix::generate_with_label(Some("user_test"), '-').unwrap();
        let encoded = id.encode();
        assert!(encoded.starts_with("user_test-"));
    }

    #[test]
    fn next_wraps_at_byte_boundary() {
        let mut bytes = [0u8; BYTES];
        bytes[30] = 0xFF;
        bytes[31] = 0xFF;
        let id = TwoFiveSix::new(bytes);
        let next = id.next();
        assert_eq!(next.as_bytes()[29], 1);
        assert_eq!(next.as_bytes()[30], 0);
        assert_eq!(next.as_bytes()[31], 0);
    }

    generate_id!(PostId, "post", '-');

    #[test]
    fn generated_type_with_hyphen_works() {
        let id = PostId::generate().unwrap();
        let encoded = id.encode();
        assert!(encoded.starts_with("post-"));
        let (label, _) = TwoFiveSix::parse_label(&encoded);
        assert_eq!(label, Some("post"));
    }

    #[test]
    fn generated_type_decode_works() {
        let id = PostId::generate().unwrap();
        let encoded = id.encode();
        let decoded = PostId::decode(&encoded).unwrap();
        assert_eq!(id, decoded);
    }

    #[test]
    fn generated_type_label_constant() {
        assert_eq!(PostId::LABEL, "post");
    }

    #[test]
    fn generated_type_separator_constant() {
        assert_eq!(PostId::SEPARATOR, '-');
    }

    #[test]
    fn error_display() {
        assert_eq!(
            format!("{}", Error::InvalidLabelCharacter('!')),
            "invalid label character: '!'"
        );
        assert_eq!(
            format!("{}", Error::InvalidLength(4)),
            "invalid length: expected 43, got 4"
        );
        assert_eq!(
            format!("{}", Error::InvalidBase64Character('!')),
            "invalid Base64 character: '!'"
        );
        assert_eq!(
            format!("{}", Error::UrandomFailure),
            "failed to read from /dev/urandom"
        );
    }

    #[test]
    fn default_is_bottom() {
        assert_eq!(TwoFiveSix::default(), TwoFiveSix::BOTTOM);
    }

    #[test]
    fn from_str_works() {
        let id_str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let id: TwoFiveSix = id_str.parse().unwrap();
        assert_eq!(id, TwoFiveSix::BOTTOM);
    }

    #[test]
    fn from_str_errs() {
        let id_str = "not a valid id";
        let err: Result<TwoFiveSix, _> = id_str.parse();
        assert!(err.is_err());
    }
}
