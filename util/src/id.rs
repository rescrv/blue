use std::fmt::Write;
use std::fs::File;
use std::io::Read;

pub const BYTES: usize = 16;

const SLICES: [(usize, usize); 5] = [(0, 4), (4, 6), (6, 8), (8, 10), (10, 16)];

/// Read a new ID from /dev/urandom
pub fn urandom() -> Option<[u8; BYTES]> {
    let mut f = match File::open("/dev/urandom") {
        Ok(f) => f,
        Err(_) => { return None; },
    };
    let mut id: [u8; BYTES] = [0u8; BYTES];
    let mut amt = 0;
    while amt < BYTES {
        let x = f.read(&mut id).ok()?;
        amt += x;
    }
    Some(id)
}

/// Encode 16B of random data in something aesthetically better.
pub fn encode(id: &[u8; BYTES]) -> String {
    let mut s = String::with_capacity(36);
    for &(start, limit) in SLICES.iter() {
        if start > 0 {
            s.push_str("-");
        }
        for i in start..limit {
            write!(&mut s, "{:02x}", id[i]).expect("unable to write to string");
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

            result[index] = (upper << 4 | lower) as u8;
            index += 1;
        }
        let dash = chars.next();
        if (limit < 16 && dash != Some('-')) || (limit == 16 && dash != None) {
            return None;
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urandom_is_nonzero() {
        assert_ne!(Some([0u8; BYTES]), urandom());
    }

    // Test that this constant does not change and document why.
    #[test]
    fn id_bytes_is_sixteen() {
        assert_eq!(BYTES, 16);
    }

    // Test that the encode-to-human-readable function does its job.
    #[test]
    fn encode_id() {
        let id = [0x55u8; BYTES];
        assert_eq!(encode(&id), "55555555-5555-5555-5555-555555555555");
    }

    // Test that the decode-from-human-readable function does its job.
    #[test]
    fn decode_id() {
        let id = [0x55u8; BYTES];
        assert_eq!(decode("55555555-5555-5555-5555-555555555555"), Some(id));
    }
}
