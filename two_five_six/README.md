two_five_six
============

A 256-bit identifier format represented as a fixed-length, 43-character URL-safe Base64 string.

TwoFiveSix combines a human-readable label with high-entropy randomness. Unlike opaque ID formats,
a TwoFiveSix ID can always be decoded back into a valid 32-byte sequence.

Features
--------

- **Fixed Length:** Always exactly 43 characters.
- **URL-Safe:** Uses RFC 4648 URL-safe Base64 alphabet (`A-Za-z0-9-_`).
- **Decodable:** Every identifier decodes to exactly 32 bytes.
- **Labeled:** Optional human-readable prefix that becomes part of the encoded bytes.
- **High Entropy:** At least 156 bits of randomness when labels stay within
  `RECOMMENDED_MAX_LABEL_LENGTH`.
- **Typed IDs:** Generate newtypes with fixed labels and checked decoding.

Usage
-----

```rust
use two_five_six::{generate_id, Error, TwoFiveSix};

// Generate a random identifier without a label.
let id = TwoFiveSix::generate().unwrap();
println!("{}", id);  // e.g., "3b6HqZ0gYtFdRsA9c4x2uE0M1n2O3P4Q5R6S7T8U"

// Generate with a label and the default "_" separator.
let id = TwoFiveSix::generate_labeled("user").unwrap();
println!("{}", id);  // e.g., "user_92wQzX1mN5rK8vJ3b6HqZ0gYtFdRsA9c4x2uE0"

// Decode back to bytes.
let bytes = id.as_bytes();

// Parse the label.
let encoded = id.encode();
let (label, suffix) = TwoFiveSix::parse_label(&encoded);
assert_eq!(label, Some("user"));
assert_eq!(suffix, id.suffix());

// Use a custom separator if you need one.
let id = TwoFiveSix::generate_with_label(Some("post"), '-').unwrap();
assert!(id.encode().starts_with("post-"));

// Validate labels before generation when accepting user-provided labels.
TwoFiveSix::validate_label("user", '_').unwrap();
assert!(matches!(
    TwoFiveSix::validate_label("user", ':'),
    Err(Error::InvalidSeparatorCharacter(':'))
));

// Generate a typed ID whose decoder checks the expected label.
generate_id!(UserId, "user", '_');
let user_id = UserId::generate().unwrap();
let decoded = UserId::decode(&user_id.encode()).unwrap();
assert_eq!(user_id, decoded);

let post_id = TwoFiveSix::generate_with_label(Some("post"), '-').unwrap();
assert!(matches!(
    UserId::decode(&post_id.encode()),
    Err(Error::InvalidLabel { .. })
));
```

Compatibility
-------------

The encoded representation remains a 43-character URL-safe Base64 string. Existing `TwoFiveSix`
constructors and parsers are still available. The user-facing changes are additive, except that
previously ambiguous invalid inputs now return errors instead of panicking or being accepted:

- non-ASCII input with a 43-byte length returns `InvalidBase64Character`;
- invalid label separators return `InvalidSeparatorCharacter`;
- labels longer than `MAX_LABEL_LENGTH` return `LabelTooLong`;
- generated typed IDs validate their configured label during `decode`, raw-byte construction, and
  untyped conversions.
