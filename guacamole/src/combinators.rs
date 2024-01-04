use super::{FromGuacamole, Guacamole};

pub fn any<T: FromGuacamole<()>>(guac: &mut Guacamole) -> T {
    T::from_guacamole(&mut (), guac)
}

pub fn from<T: FromGuacamole<U>, U>(u: &mut U) -> impl FnMut(&mut Guacamole) -> T + '_ {
    |guac| T::from_guacamole(u, guac)
}

pub fn coin() -> impl FnMut(&mut Guacamole) -> bool {
    |guac| (u8::from_guacamole(&mut (), guac) & 0x1) != 0
}

pub fn prob(p: f32) -> impl FnMut(&mut Guacamole) -> bool {
    move |guac| f32::from_guacamole(&mut (), guac) < p
}

pub fn option<P: FnMut(&mut Guacamole) -> bool, F: FnMut(&mut Guacamole) -> T, T>(
    mut pred: P,
    mut func: F,
) -> impl FnMut(&mut Guacamole) -> Option<T> {
    move |guac| {
        if pred(guac) {
            Some(func(guac))
        } else {
            None
        }
    }
}

pub fn constant<T: Clone>(t: T) -> impl FnMut(&mut Guacamole) -> T {
    move |_| t.clone()
}

pub trait RangeTo: Copy {
    fn multiply(x: Self, limit: Self) -> Self;
}

impl RangeTo for u8 {
    fn multiply(x: u8, limit: u8) -> u8 {
        (((x as u16) * (limit as u16)) >> 8) as u8
    }
}

impl RangeTo for u16 {
    fn multiply(x: u16, limit: u16) -> u16 {
        (((x as u32) * (limit as u32)) >> 16) as u16
    }
}

impl RangeTo for u32 {
    fn multiply(x: u32, limit: u32) -> u32 {
        (((x as u64) * (limit as u64)) >> 32) as u32
    }
}

impl RangeTo for u64 {
    fn multiply(x: u64, limit: u64) -> u64 {
        (((x as u128) * (limit as u128)) >> 64) as u64
    }
}

impl RangeTo for usize {
    fn multiply(x: usize, limit: usize) -> usize {
        (((x as u128) * (limit as u128)) >> usize::BITS) as usize
    }
}

pub fn range_to<R: RangeTo + FromGuacamole<()>>(limit: R) -> impl FnMut(&mut Guacamole) -> R {
    move |guac| {
        let x = R::from_guacamole(&mut (), guac);
        R::multiply(x, limit)
    }
}

pub fn uniform<
    R: RangeTo + std::ops::Add<Output = R> + std::ops::Sub<Output = R> + FromGuacamole<()>,
>(
    start: R,
    limit: R,
) -> impl FnMut(&mut Guacamole) -> R {
    let mut delta_func = range_to(limit - start);
    move |guac| start + delta_func(guac)
}

pub fn set_element<M: FnMut(&mut Guacamole) -> usize, F: FnMut(usize) -> T, T>(
    mut membership: M,
    mut func: F,
) -> impl FnMut(&mut Guacamole) -> T {
    move |guac| func(membership(guac))
}

pub fn to_vec<L: FnMut(&mut Guacamole) -> usize, F: FnMut(&mut Guacamole) -> T, T>(
    mut length: L,
    mut func: F,
) -> impl FnMut(&mut Guacamole) -> Vec<T> {
    move |guac| {
        let sz = length(guac);
        let mut collection = Vec::with_capacity(sz);
        for _ in 0..sz {
            collection.push(func(guac));
        }
        collection
    }
}

pub fn map<F: FnMut(&mut Guacamole) -> T, M: FnMut(T) -> U, T, U>(
    mut gen: F,
    mut map: M,
) -> impl FnMut(&mut Guacamole) -> U {
    move |guac| map(gen(guac))
}

pub fn filter<F: FnMut(&mut Guacamole) -> T, P: FnMut(&T) -> bool, T>(
    mut gen: F,
    mut pred: P,
) -> impl FnMut(&mut Guacamole) -> T {
    move |guac| loop {
        let t = gen(guac);
        if pred(&t) {
            return t;
        }
    }
}

pub fn select<'a, O: FnMut(&mut Guacamole) -> usize + 'a, T: Clone>(
    mut offset: O,
    values: &'a [T],
) -> impl FnMut(&mut Guacamole) -> T + 'a {
    move |guac| {
        let x = offset(guac);
        values[x].clone()
    }
}

pub fn uuid(guac: &mut Guacamole) -> String {
    use std::fmt::Write;
    let mut id = [0u8; 16];
    guac.generate(&mut id);
    // Borrowed from one_two_eight.  Used with permission.
    const SLICES: [(usize, usize); 5] = [(0, 4), (4, 6), (6, 8), (8, 10), (10, 16)];
    let mut s = String::with_capacity(36);
    for &(start, limit) in SLICES.iter() {
        if start > 0 {
            s.push('-');
        }
        for c in &id[start..limit] {
            write!(&mut s, "{:02x}", c).expect("should be able to write to string");
        }
    }
    s
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combinator_any() {
        let mut g = Guacamole::default();
        assert_eq!(12u8, any(&mut g));
        assert_eq!(237u8, any(&mut g));
        assert_eq!(89u8, any(&mut g));
        assert_eq!(79u8, any(&mut g));
    }

    #[test]
    fn combinator_from() {
        let mut g = Guacamole::default();
        assert_eq!(12u8, from(&mut ())(&mut g));
        assert_eq!(237u8, from(&mut ())(&mut g));
        assert_eq!(89u8, from(&mut ())(&mut g));
        assert_eq!(79u8, from(&mut ())(&mut g));
    }

    #[test]
    fn combinator_coin() {
        let mut g = Guacamole::default();
        assert!(!coin()(&mut g));
        assert!(coin()(&mut g));
        assert!(coin()(&mut g));
        assert!(coin()(&mut g));
        assert!(!coin()(&mut g));
        assert!(coin()(&mut g));
        assert!(coin()(&mut g));
        assert!(!coin()(&mut g));
    }

    #[test]
    fn combinator_prob() {
        let mut g = Guacamole::default();
        let mut count = 0;
        for _ in 0..1000 {
            if prob(0.75)(&mut g) {
                count += 1
            }
        }
        assert_eq!(769, count);
    }

    #[test]
    fn combinator_option() {
        let mut g = Guacamole::default();
        assert_eq!(None, option(coin(), any::<u8>)(&mut g));
        assert_eq!(Some(89u8), option(coin(), any::<u8>)(&mut g));
        assert_eq!(Some(182u8), option(coin(), any::<u8>)(&mut g));
        assert_eq!(Some(75u8), option(coin(), any::<u8>)(&mut g));
        assert_eq!(None, option(coin(), any::<u8>)(&mut g));
        assert_eq!(None, option(coin(), any::<u8>)(&mut g));
        assert_eq!(Some(143u8), option(coin(), any::<u8>)(&mut g));
        assert_eq!(Some(213u8), option(coin(), any::<u8>)(&mut g));
    }

    #[test]
    fn combinator_range_to() {
        let mut g = Guacamole::default();
        assert_eq!(57u64, range_to(64)(&mut g));
        assert_eq!(18u64, range_to(128)(&mut g));
        assert_eq!(56u64, range_to(256)(&mut g));
        assert_eq!(361u64, range_to(512)(&mut g));
    }

    #[test]
    fn combinator_uniform() {
        let mut g = Guacamole::default();
        assert_eq!(121u64, uniform(64, 128)(&mut g));
        assert_eq!(146u64, uniform(128, 256)(&mut g));
        assert_eq!(312u64, uniform(256, 512)(&mut g));
        assert_eq!(873u64, uniform(512, 1024)(&mut g));
    }

    #[test]
    fn combinator_set_element() {
        let mut g = Guacamole::default();
        assert_eq!(57usize, set_element(range_to(64usize), |x| x)(&mut g));
        assert_eq!(18usize, set_element(range_to(128usize), |x| x)(&mut g));
        assert_eq!(56usize, set_element(range_to(256usize), |x| x)(&mut g));
        assert_eq!(361usize, set_element(range_to(512usize), |x| x)(&mut g));
    }

    #[test]
    fn combinator_to_vec() {
        let mut g = Guacamole::default();
        assert_eq!(
            vec![28, 141, 143, 241, 213, 150, 53],
            to_vec(range_to(8usize), |guac| u8::from_guacamole(&mut (), guac))(&mut g)
        );
        assert_eq!(
            vec![56, 205, 209, 246, 41, 18],
            to_vec(range_to(8usize), |guac| u8::from_guacamole(&mut (), guac))(&mut g)
        );
    }

    #[test]
    fn combinator_map() {
        let mut g = Guacamole::default();
        assert_eq!(
            Some(57usize),
            map(set_element(range_to(64usize), |x| x), Option::Some)(&mut g)
        );
        assert_eq!(
            Some(18usize),
            map(set_element(range_to(128usize), |x| x), Option::Some)(&mut g)
        );
        assert_eq!(
            Some(56usize),
            map(set_element(range_to(256usize), |x| x), Option::Some)(&mut g)
        );
        assert_eq!(
            Some(361usize),
            map(set_element(range_to(512usize), |x| x), Option::Some)(&mut g)
        );
    }

    #[test]
    fn combinator_filter() {
        let mut g = Guacamole::default();
        assert_eq!(
            460usize,
            filter(set_element(range_to(512usize), |x| x), |x| *x >= 256)(&mut g)
        );
        assert_eq!(
            361usize,
            filter(set_element(range_to(512usize), |x| x), |x| *x >= 256)(&mut g)
        );
        assert_eq!(
            381usize,
            filter(set_element(range_to(512usize), |x| x), |x| *x >= 256)(&mut g)
        );
    }

    #[test]
    fn combinator_select() {
        let mut g = Guacamole::default();
        assert_eq!('C', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
        assert_eq!('A', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
        assert_eq!('A', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
        assert_eq!('C', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
        assert_eq!('A', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
        assert_eq!('C', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
        assert_eq!('C', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
        assert_eq!('B', select(range_to(3usize), &['A', 'B', 'C'])(&mut g));
    }

    #[test]
    fn combinator_uuid() {
        let mut g = Guacamole::default();
        assert_eq!("0ced594f-b619-4be6-1c8d-8ff1d5963525", uuid(&mut g));
        assert_eq!("08040784-ad6a-cb38-cdd1-f62912dab7b4", uuid(&mut g));
        assert_eq!("78b071bd-beab-020c-7af8-ad2c7f66b2be", uuid(&mut g));
        assert_eq!("35db3a02-5295-bff3-eca8-38f030f04457", uuid(&mut g));
    }

    #[test]
    fn weighted_literal() {
        #[derive(Clone, Debug, Eq, PartialEq)]
        enum Count {
            One,
            Two,
            Three,
        }
        let func = super::super::weighted! {
            0.5 => {
                Count::One
            }
            0.25 => {
                Count::Two
            }
            0.25 => {
                Count::Three
            }
        };
        let mut g = Guacamole::default();
        assert_eq!(Count::One, func(&mut g));
        assert_eq!(Count::One, func(&mut g));
        assert_eq!(Count::One, func(&mut g));
        assert_eq!(Count::Three, func(&mut g));
        assert_eq!(Count::One, func(&mut g));
        assert_eq!(Count::One, func(&mut g));
        assert_eq!(Count::Two, func(&mut g));
        assert_eq!(Count::One, func(&mut g));
        assert_eq!(Count::Three, func(&mut g));
        assert_eq!(Count::Three, func(&mut g));
    }
}
