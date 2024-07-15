#![allow(dead_code)]

////////////////////////////////////////// gamma encoding //////////////////////////////////////////

fn gamma(x: u64) -> (u128, usize) {
    assert!(x < u64::MAX);
    let x: u64 = x + 1;
    let zeros: u32 = x.leading_zeros();
    let width: u32 = 64 - zeros;
    assert!(width > 0);
    let x: u64 = x.reverse_bits();
    let mut x: u128 = x.into();
    x >>= zeros;
    x <<= width;
    (x, width as usize * 2)
}

fn ungamma(x: u128) -> (u64, usize) {
    let width = x.trailing_zeros();
    let x: u64 = (x >> width) as u64;
    let x: u64 = x.reverse_bits();
    ((x >> (64 - width)) - 1, 2 * width as usize)
}

////////////////////////////////////////// delta encoding //////////////////////////////////////////

fn delta(x: u64) -> (u128, usize) {
    let zeros: u32 = x.leading_zeros();
    let width = 64 - zeros;
    let (mut gamma, gamma_width) = gamma(width.into());
    if width == 0 {
        (gamma, gamma_width)
    } else {
        let width: usize = width as usize - 1;
        let x = x & !(1 << width);
        let x: u128 = x.into();
        gamma |= x << gamma_width;
        (gamma, gamma_width + width)
    }
}

fn undelta(x: u128) -> (u64, usize) {
    let (width, consumed) = ungamma(x);
    if width == 0 {
        (0, consumed)
    } else {
        let x: u64 = (x >> consumed) as u64;
        (x | (1 << (width - 1)), width as usize + consumed - 1)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_ungamma() {
        fn round_trip(x: u64, exp_g: u128, exp_w: usize) {
            let (ret_g, ret_w) = gamma(x);
            assert_eq!(exp_g, ret_g);
            assert_eq!(exp_w, ret_w);
            let (ret_x, ret_w) = ungamma(exp_g);
            assert_eq!(x, ret_x);
            assert_eq!(exp_w, ret_w);
        }
        round_trip(0, 2, 2);
        round_trip(1, 4, 4);
        round_trip(2, 12, 4);
        round_trip(3, 8, 6);
        round_trip(4, 40, 6);
        round_trip(5, 24, 6);
        round_trip(7, 16, 8);
        round_trip(8, 144, 8);
        round_trip(9, 80, 8);
        round_trip(15, 32, 10);
        round_trip(16, 544, 10);
        round_trip(17, 288, 10);
        round_trip(31, 64, 12);
        round_trip(32, 2112, 12);
        round_trip(33, 1088, 12);
        round_trip(63, 128, 14);
        round_trip(64, 8320, 14);
        round_trip(65, 4224, 14);
        round_trip(127, 256, 16);
        round_trip(128, 33024, 16);
        round_trip(129, 16640, 16);
        round_trip(255, 512, 18);
        round_trip(256, 131584, 18);
        round_trip(257, 66048, 18);

        round_trip(65535, 131072, 34);
        round_trip(65536, 8590065664, 34);
        round_trip(65537, 4295098368, 34);

        round_trip(16777215, 33554432, 50);
        round_trip(16777216, 562949986975744, 50);
        round_trip(16777217, 281475010265088, 50);

        round_trip(4294967295, 8589934592, 66);
        round_trip(4294967296, 36893488156009037824, 66);
        round_trip(4294967297, 18446744082299486208, 66);

        round_trip(1099511627775, 2199023255552, 82);
        round_trip(1099511627776, 2417851639231457372667904, 82);
        round_trip(1099511627777, 1208925819616828197961728, 82);

        round_trip(281474976710655, 562949953421312, 98);
        round_trip(281474976710656, 158456325028529238137041321984, 98);
        round_trip(281474976710657, 79228162514264900543497371648, 98);

        round_trip(72057594037927935, 144115188075855872, 114);
        round_trip(72057594037927936, 10384593717069655401176180734296064, 114);
        round_trip(72057594037927937, 5192296858534827772645684405075968, 114);

        round_trip(
            18446744073709551614,
            340282366920938463444927863358058659840,
            128,
        );
    }

    #[test]
    #[should_panic]
    fn gamma_limit() {
        gamma(18446744073709551615);
    }

    #[test]
    fn delta_undelta() {
        fn round_trip(x: u64, exp_d: u128, exp_w: usize) {
            let (ret_d, ret_w) = delta(x);
            assert_eq!(exp_d, ret_d);
            assert_eq!(exp_w, ret_w);
            let (ret_x, ret_w) = undelta(ret_d);
            assert_eq!(x, ret_x);
            assert_eq!(exp_w, ret_w);
        }
        round_trip(1, 4, 4);
        round_trip(2, 12, 5);
        round_trip(3, 28, 5);
        round_trip(4, 8, 8);
        round_trip(5, 72, 8);
        round_trip(7, 200, 8);
        round_trip(8, 40, 9);
        round_trip(9, 104, 9);
        round_trip(15, 488, 9);
        round_trip(16, 24, 10);
        round_trip(17, 88, 10);
        round_trip(31, 984, 10);
        round_trip(32, 56, 11);
        round_trip(33, 120, 11);

        round_trip(63, 2040, 11);
        round_trip(64, 16, 14);
        round_trip(65, 272, 14);

        round_trip(127, 16144, 14);
        round_trip(128, 144, 15);
        round_trip(129, 400, 15);

        round_trip(255, 32656, 15);
        round_trip(256, 80, 16);
        round_trip(257, 336, 16);

        round_trip(65535, 33553952, 25);
        round_trip(65536, 288, 26);
        round_trip(65537, 1312, 26);

        round_trip(16777215, 8589934176, 33);
        round_trip(16777216, 352, 34);
        round_trip(16777217, 1376, 34);

        round_trip(4294967295, 8796093020224, 43);
        round_trip(4294967296, 1088, 44);
        round_trip(4294967297, 5184, 44);

        round_trip(1099511627775, 2251799813683520, 51);
        round_trip(1099511627776, 1344, 52);
        round_trip(1099511627777, 5440, 52);

        round_trip(281474976710655, 576460752303421632, 59);
        round_trip(281474976710656, 1216, 60);
        round_trip(281474976710657, 5312, 60);

        round_trip(72057594037927935, 147573952589676411328, 67);
        round_trip(72057594037927936, 1472, 68);
        round_trip(72057594037927937, 5568, 68);

        round_trip(18446744073709551614, 151115727451828646813824, 77);
        round_trip(18446744073709551615, 151115727451828646830208, 77);
    }
}
