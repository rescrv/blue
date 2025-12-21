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
- **High Entropy:** Minimum 156 bits of randomness (with max recommended label length).

Usage
-----

```rust
use two_five_six::TwoFiveSix;

// Generate a random identifier without a label.
let id = TwoFiveSix::generate().unwrap();
println!("{}", id);  // e.g., "3b6HqZ0gYtFdRsA9c4x2uE0M1n2O3P4Q5R6S7T8U"

// Generate with a label.
let id = TwoFiveSix::generate_with_label(Some("user"), '_').unwrap();
println!("{}", id);  // e.g., "user_92wQzX1mN5rK8vJ3b6HqZ0gYtFdRsA9c4x2uE0"

// Decode back to bytes.
let bytes = id.as_bytes();

// Parse the label.
let encoded = id.encode();
let (label, suffix) = TwoFiveSix::parse_label(&encoded);
assert_eq!(label, Some("user"));
```
