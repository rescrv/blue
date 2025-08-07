//! This module includes a set of combinators that make it easy to construct complex types using
//! simple statements.

use std::time::Duration;

use super::{FromGuacamole, Guacamole};

/// any produces an item of any type, so long as it implements FromGuacamole<()>.
pub fn any<T: FromGuacamole<()>>(guac: &mut Guacamole) -> T {
    T::from_guacamole(&mut (), guac)
}

/// from produces an item from the provided parameters, so long as the target type implements
/// FromGuacamole for the appropriate type.
pub fn from<T: FromGuacamole<U>, U>(u: &mut U) -> impl FnMut(&mut Guacamole) -> T + '_ {
    |guac| T::from_guacamole(u, guac)
}

/// A fair coin toss.
pub fn coin() -> impl FnMut(&mut Guacamole) -> bool {
    |guac| (u8::from_guacamole(&mut (), guac) & 0x1) != 0
}

/// Returns true with probability p.
pub fn prob(p: f32) -> impl FnMut(&mut Guacamole) -> bool {
    move |guac| f32::from_guacamole(&mut (), guac) < p
}

/// Use the first function to tell whether to generate a Some type using the second function.
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

/// Returns a constant and does not consume any guacamole.
pub fn constant<T: Clone>(t: T) -> impl FnMut(&mut Guacamole) -> T {
    move |_| t.clone()
}

/// A helper type for [range_to].
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

/// Return a number in the closed-open interval [0, limit).
pub fn range_to<R: RangeTo + FromGuacamole<()>>(limit: R) -> impl FnMut(&mut Guacamole) -> R {
    move |guac| {
        let x = R::from_guacamole(&mut (), guac);
        R::multiply(x, limit)
    }
}

/// Take a function that guacamole and returns a usize, and couple it to a function that takes a
/// usize and returns an arbitrary type.
///
/// This is useful for generating sets of elements.  Use [range_to] or [unique_set] as the first
/// argument, and [any] (or anything else) as the second argument to quickly and easily generate a
/// random, finite set of elements according to the distribution of the first argument.
///
/// This is *the* motivating use case for guacamole and why it was originally mashed.
pub fn set_element<M: FnMut(&mut Guacamole) -> usize, F: FnMut(usize) -> T, T>(
    mut membership: M,
    mut func: F,
) -> impl FnMut(&mut Guacamole) -> T {
    move |guac| func(membership(guac))
}

/// Create a vector with prescribed length and element generation.
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

/// Map the type returned by a function that takes guacamole to another type.
pub fn map<F: FnMut(&mut Guacamole) -> T, M: FnMut(T) -> U, T, U>(
    mut gen: F,
    mut map: M,
) -> impl FnMut(&mut Guacamole) -> U {
    move |guac| map(gen(guac))
}

/// Filter values returned by guacamole, returning the first generated value that matches the
/// prescribed predicate.
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

/// Select values from a slice, using an offset function to select the slice.
pub fn select<'a, O: FnMut(&mut Guacamole) -> usize + 'a, T: Clone>(
    mut offset: O,
    values: &'a [T],
) -> impl FnMut(&mut Guacamole) -> T + 'a {
    move |guac| {
        let x = offset(guac);
        values[x].clone()
    }
}

/// Generate a UUID using guacamole.
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
            write!(&mut s, "{c:02x}").expect("should be able to write to string");
        }
    }
    s
}

/// Return the non-negative integers in increasing order, consuming no guacamole.
pub fn enumerate() -> impl FnMut(&mut Guacamole) -> usize {
    let mut x = 0;
    move |_| {
        let ret = x;
        x += 1;
        ret
    }
}

/// Given a function that takes guacamole and returns an arbitrary type, create a function that
/// takes a usize as the seed to guacamole and returns an arbitrary value generated from a new
/// guacamole stream on that seed.
///
/// This works well with [set_element] to allow construction of arbitrary sets of data.
pub fn from_seed<T, F: FnMut(&mut Guacamole) -> T>(mut func: F) -> impl FnMut(usize) -> T {
    move |index| {
        let mut g = Guacamole::new(index as u64);
        func(&mut g)
    }
}

/// Create a unique set-generating function.  This takes numbers X in the range [0, usize) and
/// returns X * random + random.  Random should be selected to be a prime number far apart from
/// other prime numbers provided to unique_set.  On platforms with 64-bit usize, a 63-bit number
/// works well.  On platforms with 32-bit usize, a 31-bit number works well.
pub fn unique_set(set_size: usize, random: usize) -> impl FnMut(&mut Guacamole) -> usize {
    let mut indexer = unique_set_index(random);
    move |guac| indexer(range_to(set_size)(guac))
}

/// Index into a unique set.  Converts numbers in [0, set_size) into X * random + random.  Random
/// should be a prime number far apart from other prime numbers provided to unique_set.  On
/// platforms with 64-bit usize, a 63-bit number works well.  On platforms with 32-bit usize, a
/// 31-bit number works well.  Nothing prevents set_size from varying in size once this is
/// instantiated.
pub fn unique_set_index(random: usize) -> impl FnMut(usize) -> usize {
    move |index| index.wrapping_mul(random).wrapping_add(random)
}

/// Generate numbers uniformly distributed between start and limit.
pub fn uniform<
    R: RangeTo + std::ops::Add<Output = R> + std::ops::Sub<Output = R> + FromGuacamole<()>,
>(
    start: R,
    limit: R,
) -> impl FnMut(&mut Guacamole) -> R {
    let mut delta_func = range_to(limit - start);
    move |guac| start + delta_func(guac)
}

/// Use the Box-Muller transform to generate normal numbers with the prescribed mean and standard
/// deviation.
pub fn normal(mean: f64, stdev: f64) -> impl FnMut(&mut Guacamole) -> f64 {
    move |guac| {
        // Box-Muller transform:
        // https://en.wikipedia.org/wiki/Box%E2%80%93Muller_transform
        // We return half the numbers we could generate.
        const TWO_PI: f64 = std::f64::consts::PI * 2.0;
        let mut u1: f64 = 0.0;
        while u1 <= 0.0 {
            u1 = any::<f64>(guac);
        }
        let u2 = any::<f64>(guac);
        let mag = stdev * (-2.0 * u1.ln()).sqrt();
        mag * (TWO_PI * u2).cos() + mean
    }
}

/// Generate numbers according to a poisson distribution with the specified interarrival rate.
pub fn exponentially_distributed(mean: impl Into<f64>) -> impl FnMut(&mut Guacamole) -> f64 {
    let mean = mean.into();
    move |guac| (0.0 - f64::from_guacamole(&mut (), guac).ln()) * mean
}

/// Generate numbers according to a poisson distribution with the specified interarrival rate.
pub fn poisson(interarrival_rate: impl Into<f64>) -> impl FnMut(&mut Guacamole) -> f64 {
    let interarrival_rate = interarrival_rate.into();
    move |guac| exponentially_distributed(1.0 / interarrival_rate)(guac)
}

/// Generate a duration that, if perfectly respected, corresponds to a poisson distribution of
/// arrivals with the specified interarrival rate.
pub fn interarrival_duration(interarrival_rate: f64) -> impl FnMut(&mut Guacamole) -> Duration {
    move |guac| {
        map(poisson(interarrival_rate), |x| {
            Duration::from_micros((x * 1_000_000.0) as u64)
        })(guac)
    }
}

/// Generate a string using the provided length-determining and bytes-converting functions.
pub fn string(
    mut length: impl FnMut(&mut Guacamole) -> usize,
    mut convert: impl FnMut(&[u8]) -> String,
) -> impl FnMut(&mut Guacamole) -> String {
    let mut buffer = vec![];
    move |guac| {
        let len = length(guac);
        buffer.resize(len, 0);
        guac.generate(&mut buffer[..len]);
        convert(&buffer[..len])
    }
}

/// The lower character set includes lower-case ASCII alphabets.
pub const CHAR_SET_LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
/// The upper character set includes upper-case ASCII alphabets.
pub const CHAR_SET_UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The alph character set includes lower- and upper-case ASCII alphabets.
pub const CHAR_SET_ALPHA: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The alph character set includes lower- and upper-case ASCII alphabets and a space.
pub const CHAR_SET_ALPHA_SPACE8: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ        ";
/// The digit character set includes ASCII digits.
pub const CHAR_SET_DIGIT: &str = "0123456789";
/// The alnum character set includes lower- and upper-case ASCII alphabets and the digits.
pub const CHAR_SET_ALNUM: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
/// The punct character set includes ASCII punctuation.
pub const CHAR_SET_PUNCT: &str = "!\"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~";
/// The hex character set includes lower-case hexadecimal numbers.
pub const CHAR_SET_HEX: &str = "0123456789abcdef";
/// The default character set includes most printable ASCII.
pub const CHAR_SET_DEFAULT: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!\"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~";
/// The base-20 characters used for Plus Codes (Open Location Codes).
pub const CHAR_SET_PLUS_CODES: &str = "23456789CFGHJMPQRVWX";

/// Create a function that maps a slice of bytes to a string of the same length derived from the
/// provided charset.  Requires that chars be fewer than 256 characters.
///
/// # Panics
///
/// - If chars.len() > 256.
pub fn to_charset(chars: &str) -> impl FnMut(&[u8]) -> String {
    let s: Vec<char> = chars.chars().collect();
    assert!(s.len() <= 256);
    let mut table: [char; 256] = ['A'; 256];
    for (i, x) in table.iter_mut().enumerate() {
        let d: f64 = (i as f64) / 256.0 * s.len() as f64;
        let d: usize = d as usize;
        assert!(d < s.len());
        *x = s[d];
    }
    move |bytes: &[u8]| {
        let mut string = String::with_capacity(bytes.len());
        for b in bytes.iter() {
            string.push(table[*b as usize]);
        }
        string
    }
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

    #[test]
    fn string() {
        let mut strings = super::string(super::uniform(1, 8), to_charset(super::CHAR_SET_DEFAULT));
        let mut g = Guacamole::default();
        assert_eq!("kZ0_;3t", strings(&mut g));
        assert_eq!("u./{pg", strings(&mut g));
        assert_eq!("!aeS|\"", strings(&mut g));
        assert_eq!("aE", strings(&mut g));
    }
}
