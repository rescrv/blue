const DIVIDE_32: u32 = 0x80000000u32;
const DIVIDE_64: u64 = 0x8000000000000000u64;

pub fn encode_i32(x: i32) -> u32 {
    let offset = if x >= 0 { DIVIDE_32 } else { i32::MIN as u32 };
    (x as u32).wrapping_add(offset)
}

pub fn decode_i32(x: u32) -> i32 {
    let offset = if x >= DIVIDE_32 {
        DIVIDE_32
    } else {
        i32::MIN as u32
    };
    x.wrapping_sub(offset) as i32
}

pub fn encode_i64(x: i64) -> u64 {
    let offset = if x >= 0 { DIVIDE_64 } else { i64::MIN as u64 };
    (x as u64).wrapping_add(offset)
}

pub fn decode_i64(x: u64) -> i64 {
    let offset = if x >= DIVIDE_64 {
        DIVIDE_64
    } else {
        i64::MIN as u64
    };
    x.wrapping_sub(offset) as i64
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i64_encode() {
        assert_eq!(0x0000000000000000u64, encode_i64(i64::MIN));
        assert_eq!(0x0000000000000001u64, encode_i64(i64::MIN + 1));
        assert_eq!(0x7fffffffffffffffu64, encode_i64(-1));
        assert_eq!(0x8000000000000000u64, encode_i64(0));
        assert_eq!(0x8000000000000001u64, encode_i64(1));
        assert_eq!(0xfffffffffffffffeu64, encode_i64(i64::MAX - 1));
        assert_eq!(0xffffffffffffffffu64, encode_i64(i64::MAX));
    }

    #[test]
    fn i64_decode() {
        assert_eq!(decode_i64(0x0000000000000000u64), i64::MIN);
        assert_eq!(decode_i64(0x0000000000000001u64), i64::MIN + 1);
        assert_eq!(decode_i64(0x7fffffffffffffffu64), -1);
        assert_eq!(decode_i64(0x8000000000000000u64), 0);
        assert_eq!(decode_i64(0x8000000000000001u64), 1);
        assert_eq!(decode_i64(0xfffffffffffffffeu64), i64::MAX - 1);
        assert_eq!(decode_i64(0xffffffffffffffffu64), i64::MAX);
    }

    #[test]
    fn i32_encode() {
        assert_eq!(0x00000000u32, encode_i32(i32::MIN));
        assert_eq!(0x00000001u32, encode_i32(i32::MIN + 1));
        assert_eq!(0x7fffffffu32, encode_i32(-1));
        assert_eq!(0x80000000u32, encode_i32(0));
        assert_eq!(0x80000001u32, encode_i32(1));
        assert_eq!(0xfffffffeu32, encode_i32(i32::MAX - 1));
        assert_eq!(0xffffffffu32, encode_i32(i32::MAX));
    }

    #[test]
    fn i32_decode() {
        assert_eq!(decode_i32(0x00000000u32), i32::MIN);
        assert_eq!(decode_i32(0x00000001u32), i32::MIN + 1);
        assert_eq!(decode_i32(0x7fffffffu32), -1);
        assert_eq!(decode_i32(0x80000000u32), 0);
        assert_eq!(decode_i32(0x80000001u32), 1);
        assert_eq!(decode_i32(0xfffffffeu32), i32::MAX - 1);
        assert_eq!(decode_i32(0xffffffffu32), i32::MAX);
    }
}
