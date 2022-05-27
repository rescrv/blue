/// zigzag implements the signed integer encoding format described in the protobuf encoding
/// document.  The format allows representing signed integers in a way that is sympathetic to the
/// varint encoding format.  Negative values i output -2i-1; positive values of i output 2i.

// I consider this an appropriate amount of documentation for this module because we mathematically
// specified the behavior above and the type signature should be sufficient for someone to
// understand exactly the behavior they are getting.

pub fn zigzag(x: i64) -> u64 {
    ((x << 1) ^ (x >> 63)) as u64
}

pub fn unzigzag(x: u64) -> i64 {
    ((x >> 1) as i64) ^ (-((x & 1) as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_case(s: i64, u: u64) {
        assert_eq!(u, zigzag(s), "human broke zigzag({}) -> {}?", s, u);
        assert_eq!(s, unzigzag(u), "human broke unzigzag({}) -> {}?", u, s);
    }

    #[test]
    fn zero_one_two() {
        // test the range of 32-bit inputs in 32- and 64-bit contexts
        test_case(0, 0);
        test_case(-1, 1);
        test_case(1, 2);
        test_case(-2, 3);
        test_case(2, 4);
    }

    #[test]
    fn eight_bits() {
        // test 8-bit range
        test_case(32767, 65534);
        test_case(-32768, 65535);
    }

    #[test]
    fn sixteen_bits() {
        // test 16-bit range
        test_case(32767, 65534);
        test_case(-32768, 65535);
    }

    #[test]
    fn thirty_two_bits() {
        // test 32-bit range
        test_case(2147483647, 4294967294);
        test_case(-2147483648, 4294967295);
    }

    #[test]
    fn sixty_four_bits() {
        // test 64-bit range
        test_case(9223372036854775807, 18446744073709551614);
        test_case(-9223372036854775808, 18446744073709551615);
    }
}
