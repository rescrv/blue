//! Compose deterministic generators from smaller sampling functions.
//!
//! The core shape in this module is `FnMut(&mut Guacamole) -> T`: a stateful function that draws
//! from a [`Guacamole`] stream and yields a value of type `T`.  Some combinators consume bytes
//! from the stream, while others only adapt or combine existing generators without introducing
//! additional randomness.
//!
//! # Examples
//!
//! ```rust
//! use guacamole::Guacamole;
//! use guacamole::combinators::{
//!     any, constant, flat_map, pair, range_to, repeat, string, to_charset, CHAR_SET_HEX,
//! };
//!
//! let mut guac = Guacamole::new(7);
//! let mut item = pair(any::<u8>, constant("tag"));
//! let (value, label) = item(&mut guac);
//! assert!(value <= u8::MAX);
//! assert_eq!(label, "tag");
//!
//! let mut token = string(constant(6usize), to_charset(CHAR_SET_HEX));
//! let token = token(&mut guac);
//! assert_eq!(6, token.len());
//! assert!(token.chars().all(|ch| CHAR_SET_HEX.contains(ch)));
//!
//! let mut payload = flat_map(range_to(4usize), |len| repeat(len + 1, any::<u8>));
//! let payload = payload(&mut guac);
//! assert!((1..=4).contains(&payload.len()));
//! ```

use std::time::Duration;

use super::{FromGuacamole, Guacamole};

/// Draw a value directly from the stream using `T`'s [`FromGuacamole<()>`] implementation.
pub fn any<T: FromGuacamole<()>>(guac: &mut Guacamole) -> T {
    T::from_guacamole(&mut (), guac)
}

/// Bind generator parameters once and expose them as a stream-consuming closure.
pub fn from<T: FromGuacamole<U>, U>(u: &mut U) -> impl FnMut(&mut Guacamole) -> T + '_ {
    |guac| T::from_guacamole(u, guac)
}

/// Draw a fair Bernoulli sample.
pub fn coin() -> impl FnMut(&mut Guacamole) -> bool {
    |guac| (u8::from_guacamole(&mut (), guac) & 0x1) != 0
}

/// Draw a Bernoulli sample with threshold `p`.
///
/// Because sampled `f32` values lie in `[0, 1)`, values of `p <= 0.0` always return `false` and
/// values of `p >= 1.0` always return `true`.
pub fn prob(p: f32) -> impl FnMut(&mut Guacamole) -> bool {
    move |guac| f32::from_guacamole(&mut (), guac) < p
}

/// Conditionally sample a value.
///
/// The predicate is evaluated first.  The value generator is called only when the predicate
/// returns `true`.  For a total branch with a fallback, use [`either`].
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

/// Choose exactly one of two generators.
///
/// The predicate is evaluated first.  Only the selected branch consumes the remaining stream.
///
/// # Examples
///
/// ```rust
/// use guacamole::Guacamole;
/// use guacamole::combinators::{constant, either, prob};
///
/// let mut guac = Guacamole::new(5);
/// let mut label = either(prob(0.0), constant("left"), constant("right"));
/// assert_eq!("right", label(&mut guac));
/// ```
pub fn either<
    P: FnMut(&mut Guacamole) -> bool,
    L: FnMut(&mut Guacamole) -> T,
    R: FnMut(&mut Guacamole) -> T,
    T,
>(
    mut pred: P,
    mut left: L,
    mut right: R,
) -> impl FnMut(&mut Guacamole) -> T {
    move |guac| {
        if pred(guac) {
            left(guac)
        } else {
            right(guac)
        }
    }
}

/// Repeat a cloned constant without consuming the stream.
pub fn constant<T: Clone>(t: T) -> impl FnMut(&mut Guacamole) -> T {
    move |_| t.clone()
}

/// Scale uniformly generated values into a half-open range.
///
/// This trait exists to support [`range_to`] for integer types via high-word multiplication.
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

/// Project a generated integer into the half-open interval `[0, limit)`.
///
/// A `limit` of zero always returns zero.
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

/// Collect a generated sequence into a [`Vec`].
///
/// The length is drawn first.  The element generator is then called that many times.  For a fixed
/// count known at construction time, use [`repeat`].
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

/// Collect exactly `count` generated values into a [`Vec`].
///
/// This is the fixed-length counterpart to [`to_vec`].
///
/// # Examples
///
/// ```rust
/// use guacamole::Guacamole;
/// use guacamole::combinators::{constant, repeat};
///
/// let mut guac = Guacamole::default();
/// let mut values = repeat(3, constant(7u8));
/// assert_eq!(vec![7, 7, 7], values(&mut guac));
/// ```
pub fn repeat<F: FnMut(&mut Guacamole) -> T, T>(
    count: usize,
    mut func: F,
) -> impl FnMut(&mut Guacamole) -> Vec<T> {
    move |guac| {
        let mut collection = Vec::with_capacity(count);
        for _ in 0..count {
            collection.push(func(guac));
        }
        collection
    }
}

/// Sample two generators in sequence and return both results.
pub fn pair<L: FnMut(&mut Guacamole) -> A, R: FnMut(&mut Guacamole) -> B, A, B>(
    mut left: L,
    mut right: R,
) -> impl FnMut(&mut Guacamole) -> (A, B) {
    move |guac| (left(guac), right(guac))
}

/// Collect exactly `N` generated values into an array.
///
/// # Examples
///
/// ```rust
/// use guacamole::Guacamole;
/// use guacamole::combinators::{constant, to_array};
///
/// let mut guac = Guacamole::default();
/// let mut values = to_array::<4, _, _>(constant(3u8));
/// assert_eq!([3, 3, 3, 3], values(&mut guac));
/// ```
pub fn to_array<const N: usize, F: FnMut(&mut Guacamole) -> T, T>(
    mut func: F,
) -> impl FnMut(&mut Guacamole) -> [T; N] {
    move |guac| std::array::from_fn(|_| func(guac))
}

/// Transform a generated value without consuming additional randomness.
///
/// For dependent generation where the mapped function returns another generator, use
/// [`flat_map`].
pub fn map<F: FnMut(&mut Guacamole) -> T, M: FnMut(T) -> U, T, U>(
    mut generate: F,
    mut map: M,
) -> impl FnMut(&mut Guacamole) -> U {
    move |guac| map(generate(guac))
}

/// Select a follow-on generator from a previously generated value.
///
/// This is the dependent counterpart to [`map`].  The first generator chooses parameters, and the
/// returned generator consumes the remaining stream using those parameters.
///
/// # Examples
///
/// ```rust
/// use guacamole::Guacamole;
/// use guacamole::combinators::{any, flat_map, range_to, repeat};
///
/// let mut guac = Guacamole::new(9);
/// let mut bytes = flat_map(range_to(4usize), |len| repeat(len + 1, any::<u8>));
/// let values = bytes(&mut guac);
/// assert!((1..=4).contains(&values.len()));
/// ```
pub fn flat_map<F, M, G, T, U>(mut generate: F, mut map: M) -> impl FnMut(&mut Guacamole) -> U
where
    F: FnMut(&mut Guacamole) -> T,
    M: FnMut(T) -> G,
    G: FnMut(&mut Guacamole) -> U,
{
    move |guac| {
        let mut next = map(generate(guac));
        next(guac)
    }
}

/// Repeatedly sample until a generated value satisfies the predicate.
///
/// If the predicate can never match, this combinator loops forever.
pub fn filter<F: FnMut(&mut Guacamole) -> T, P: FnMut(&T) -> bool, T>(
    mut generate: F,
    mut pred: P,
) -> impl FnMut(&mut Guacamole) -> T {
    move |guac| loop {
        let t = generate(guac);
        if pred(&t) {
            return t;
        }
    }
}

/// Clone a value from a slice using a generated offset.
///
/// For the common case of uniform selection over a non-empty slice, use [`one_of`].
///
/// # Panics
///
/// Panics if `offset` returns an index outside `values`.
pub fn select<'a, O: FnMut(&mut Guacamole) -> usize + 'a, T: Clone>(
    mut offset: O,
    values: &'a [T],
) -> impl FnMut(&mut Guacamole) -> T + 'a {
    move |guac| {
        let x = offset(guac);
        values[x].clone()
    }
}

/// Uniformly choose a cloned value from a non-empty slice.
///
/// This is [`select`] with an internal [`range_to`] over `values.len()`.
///
/// # Examples
///
/// ```rust
/// use guacamole::Guacamole;
/// use guacamole::combinators::one_of;
///
/// let mut guac = Guacamole::default();
/// let mut letter = one_of(&['A', 'B', 'C']);
/// assert!(matches!(letter(&mut guac), 'A' | 'B' | 'C'));
/// ```
///
/// # Panics
///
/// Panics if `values` is empty.
pub fn one_of<'a, T: Clone>(values: &'a [T]) -> impl FnMut(&mut Guacamole) -> T + 'a {
    assert!(!values.is_empty(), "values must not be empty");
    let mut offset = range_to(values.len());
    move |guac| values[offset(guac)].clone()
}

/// Generate a shuffled copy of a slice.
///
/// Each call clones `values` and applies a Fisher-Yates shuffle driven by the stream.
///
/// # Examples
///
/// ```rust
/// use guacamole::Guacamole;
/// use guacamole::combinators::shuffle;
///
/// let mut guac = Guacamole::default();
/// let mut permute = shuffle(&[1, 2, 3, 4]);
/// let mut values = permute(&mut guac);
/// values.sort();
/// assert_eq!(vec![1, 2, 3, 4], values);
/// ```
pub fn shuffle<'a, T: Clone>(values: &'a [T]) -> impl FnMut(&mut Guacamole) -> Vec<T> + 'a {
    move |guac| {
        let mut values = values.to_vec();
        for i in (1..values.len()).rev() {
            let j = range_to(i + 1)(guac);
            values.swap(i, j);
        }
        values
    }
}

fn format_uuid(id: &[u8; 16]) -> String {
    use std::fmt::Write;
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

/// Format 16 generated bytes as an RFC-conforming version 4 UUID.
///
/// This is shorthand for [`uuid_version::<4>`].
pub fn uuid(guac: &mut Guacamole) -> String {
    uuid_version::<4>(guac)
}

/// Format 16 generated bytes as an RFC-conforming version 4 or version 7 UUID.
///
/// The generator stays deterministic for both versions.  For version 7, the timestamp field is
/// filled from the stream rather than from wall-clock time, so the result is structurally valid as
/// UUIDv7 without carrying real-time ordering semantics.
///
/// # Panics
///
/// Panics if `VERSION` is not 4 or 7.
pub fn uuid_version<const VERSION: u8>(guac: &mut Guacamole) -> String {
    assert!(
        matches!(VERSION, 4 | 7),
        "uuid version must be either 4 or 7"
    );
    let mut id = [0u8; 16];
    guac.generate(&mut id);
    // RFC 4122 / RFC 9562 variant.
    id[8] = (id[8] & 0x3f) | 0x80;
    // Version in the high nibble of octet 6.
    id[6] = (id[6] & 0x0f) | (VERSION << 4);
    format_uuid(&id)
}

/// Produce increasing indices without consuming the stream.
pub fn enumerate() -> impl FnMut(&mut Guacamole) -> usize {
    let mut x = 0;
    move |_| {
        let ret = x;
        x += 1;
        ret
    }
}

/// Re-seed a generator for each input index.
///
/// Given a function that takes guacamole and returns an arbitrary type, create a function that
/// takes a `usize` as the seed to guacamole and returns an arbitrary value generated from a new
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
    let mut select_index = range_to(set_size);
    let mut indexer = unique_set_index(random);
    move |guac| indexer(select_index(guac))
}

/// Index into a unique set.  Converts numbers in [0, set_size) into X * random + random.  Random
/// should be a prime number far apart from other prime numbers provided to unique_set.  On
/// platforms with 64-bit usize, a 63-bit number works well.  On platforms with 32-bit usize, a
/// 31-bit number works well.  Nothing prevents set_size from varying in size once this is
/// instantiated.
pub fn unique_set_index(random: usize) -> impl FnMut(usize) -> usize {
    move |index| index.wrapping_mul(random).wrapping_add(random)
}

/// Generate values uniformly distributed in the half-open interval `[start, limit)`.
///
/// This combinator assumes `start <= limit`.
pub fn uniform<
    R: RangeTo + std::ops::Add<Output = R> + std::ops::Sub<Output = R> + FromGuacamole<()>,
>(
    start: R,
    limit: R,
) -> impl FnMut(&mut Guacamole) -> R {
    let mut delta_func = range_to(limit - start);
    move |guac| start + delta_func(guac)
}

/// Generate values from a normal distribution with the prescribed mean and standard deviation.
///
/// This uses the Box-Muller transform and consumes two floating-point samples per generated value.
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

/// Generate values from an exponential distribution with the provided mean.
///
/// The mean should be finite and non-negative.
pub fn exponentially_distributed(mean: impl Into<f64>) -> impl FnMut(&mut Guacamole) -> f64 {
    let mean = mean.into();
    move |guac| (0.0 - f64::from_guacamole(&mut (), guac).ln()) * mean
}

/// Generate interarrival times for a Poisson process with the specified rate.
///
/// The `interarrival_rate` should be positive and finite.
pub fn poisson(interarrival_rate: impl Into<f64>) -> impl FnMut(&mut Guacamole) -> f64 {
    let interarrival_rate = interarrival_rate.into();
    let mut sample = exponentially_distributed(1.0 / interarrival_rate);
    move |guac| sample(guac)
}

/// Generate a duration that, if perfectly respected, corresponds to a poisson distribution of
/// arrivals with the specified interarrival rate.
///
/// The `interarrival_rate` should be positive and finite.
pub fn interarrival_duration(interarrival_rate: f64) -> impl FnMut(&mut Guacamole) -> Duration {
    let mut sample = poisson(interarrival_rate);
    move |guac| Duration::from_micros((sample(guac) * 1_000_000.0) as u64)
}

/// Generate strings by sampling a byte slice and converting it.
///
/// The length is drawn first.  The byte buffer is then filled from the stream and passed to
/// `convert`.
pub fn string(
    mut length: impl FnMut(&mut Guacamole) -> usize,
    mut convert: impl FnMut(&[u8]) -> String,
) -> impl FnMut(&mut Guacamole) -> String {
    let mut buffer = Vec::new();
    move |guac| {
        let len = length(guac);
        buffer.resize(len, 0);
        guac.generate(&mut buffer[..len]);
        convert(&buffer[..len])
    }
}

/// Lower-case ASCII letters.
pub const CHAR_SET_LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
/// Upper-case ASCII letters.
pub const CHAR_SET_UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Lower-case and upper-case ASCII letters.
pub const CHAR_SET_ALPHA: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// Lower-case and upper-case ASCII letters with extra weight on spaces.
pub const CHAR_SET_ALPHA_SPACE8: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ        ";
/// ASCII digits.
pub const CHAR_SET_DIGIT: &str = "0123456789";
/// Lower-case and upper-case ASCII letters plus digits.
pub const CHAR_SET_ALNUM: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
/// ASCII punctuation characters.
pub const CHAR_SET_PUNCT: &str = "!\"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~";
/// Lower-case hexadecimal digits.
pub const CHAR_SET_HEX: &str = "0123456789abcdef";
/// Most printable ASCII characters.
pub const CHAR_SET_DEFAULT: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!\"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~";
/// The base-20 alphabet used by Plus Codes (Open Location Codes).
pub const CHAR_SET_PLUS_CODES: &str = "23456789CFGHJMPQRVWX";

/// Convert arbitrary bytes into characters from a fixed alphabet.
///
/// The returned closure preserves input length and remaps each byte through a 256-entry lookup
/// table derived from `chars`.  Shorter alphabets are weighted approximately evenly across the
/// byte domain.
///
/// # Examples
///
/// ```rust
/// use guacamole::combinators::{to_charset, CHAR_SET_HEX};
///
/// let mut hex = to_charset(CHAR_SET_HEX);
/// let text = hex(&[0, 127, 255]);
/// assert_eq!(3, text.len());
/// assert!(text.chars().all(|ch| CHAR_SET_HEX.contains(ch)));
/// ```
///
/// # Panics
///
/// Panics if `chars` is empty or contains more than 256 characters.
pub fn to_charset(chars: &str) -> impl FnMut(&[u8]) -> String {
    let s: Vec<char> = chars.chars().collect();
    assert!(!s.is_empty(), "charset must not be empty");
    assert!(
        s.len() <= 256,
        "charset must contain at most 256 characters"
    );
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
    fn combinator_repeat() {
        let mut expected = Guacamole::default();
        let expected1 = to_vec(constant(4usize), any::<u8>)(&mut expected);
        let expected2 = to_vec(constant(4usize), any::<u8>)(&mut expected);

        let mut g = Guacamole::default();
        let mut repeated = repeat(4, any::<u8>);
        assert_eq!(expected1, repeated(&mut g));
        assert_eq!(expected2, repeated(&mut g));
    }

    #[test]
    fn combinator_pair() {
        let mut expected = Guacamole::default();
        let expected1 = (any::<u8>(&mut expected), any::<u16>(&mut expected));
        let expected2 = (any::<u8>(&mut expected), any::<u16>(&mut expected));

        let mut g = Guacamole::default();
        let mut pairs = pair(any::<u8>, any::<u16>);
        assert_eq!(expected1, pairs(&mut g));
        assert_eq!(expected2, pairs(&mut g));
    }

    #[test]
    fn combinator_to_array() {
        let mut expected = Guacamole::default();
        let expected1 = [
            any::<u8>(&mut expected),
            any::<u8>(&mut expected),
            any::<u8>(&mut expected),
            any::<u8>(&mut expected),
        ];
        let expected2 = [
            any::<u8>(&mut expected),
            any::<u8>(&mut expected),
            any::<u8>(&mut expected),
            any::<u8>(&mut expected),
        ];

        let mut g = Guacamole::default();
        let mut arrays = to_array::<4, _, _>(any::<u8>);
        assert_eq!(expected1, arrays(&mut g));
        assert_eq!(expected2, arrays(&mut g));
    }

    #[test]
    fn combinator_either() {
        let mut expected = Guacamole::default();
        let expected1 = if coin()(&mut expected) {
            any::<u8>(&mut expected)
        } else {
            0u8
        };
        let expected2 = if coin()(&mut expected) {
            any::<u8>(&mut expected)
        } else {
            0u8
        };

        let mut g = Guacamole::default();
        let mut choose = either(coin(), any::<u8>, constant(0u8));
        assert_eq!(expected1, choose(&mut g));
        assert_eq!(expected2, choose(&mut g));
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
    fn combinator_flat_map() {
        let mut expected = Guacamole::default();
        let start1 = range_to(8usize)(&mut expected);
        let expected1 = repeat(start1 + 1, any::<u8>)(&mut expected);
        let start2 = range_to(8usize)(&mut expected);
        let expected2 = repeat(start2 + 1, any::<u8>)(&mut expected);

        let mut g = Guacamole::default();
        let mut values = flat_map(range_to(8usize), |len| repeat(len + 1, any::<u8>));
        assert_eq!(expected1, values(&mut g));
        assert_eq!(expected2, values(&mut g));
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
    fn combinator_one_of() {
        let mut expected = Guacamole::default();
        let values = ['A', 'B', 'C'];
        let expected1 = select(range_to(values.len()), &values)(&mut expected);
        let expected2 = select(range_to(values.len()), &values)(&mut expected);

        let mut g = Guacamole::default();
        let mut choose = one_of(&values);
        assert_eq!(expected1, choose(&mut g));
        assert_eq!(expected2, choose(&mut g));
    }

    #[test]
    #[should_panic]
    fn combinator_one_of_empty_panics() {
        let _ = one_of::<u8>(&[]);
    }

    #[test]
    fn combinator_shuffle() {
        let mut expected = Guacamole::default();
        let mut expected1 = vec!['A', 'B', 'C', 'D'];
        for i in (1..expected1.len()).rev() {
            expected1.swap(i, range_to(i + 1)(&mut expected));
        }
        let mut expected2 = vec!['A', 'B', 'C', 'D'];
        for i in (1..expected2.len()).rev() {
            expected2.swap(i, range_to(i + 1)(&mut expected));
        }

        let mut g = Guacamole::default();
        let mut permute = shuffle(&['A', 'B', 'C', 'D']);
        assert_eq!(expected1, permute(&mut g));
        assert_eq!(expected2, permute(&mut g));
    }

    #[test]
    fn combinator_uuid() {
        let mut g = Guacamole::default();
        assert_eq!("0ced594f-b619-4be6-9c8d-8ff1d5963525", uuid(&mut g));
        assert_eq!("08040784-ad6a-4b38-8dd1-f62912dab7b4", uuid(&mut g));
        assert_eq!("78b071bd-beab-420c-baf8-ad2c7f66b2be", uuid(&mut g));
        assert_eq!("35db3a02-5295-4ff3-aca8-38f030f04457", uuid(&mut g));
    }

    #[test]
    fn combinator_uuid_version_7() {
        let mut g = Guacamole::default();
        assert_eq!(
            "0ced594f-b619-7be6-9c8d-8ff1d5963525",
            uuid_version::<7>(&mut g)
        );
        assert_eq!(
            "08040784-ad6a-7b38-8dd1-f62912dab7b4",
            uuid_version::<7>(&mut g)
        );
        assert_eq!(
            "78b071bd-beab-720c-baf8-ad2c7f66b2be",
            uuid_version::<7>(&mut g)
        );
        assert_eq!(
            "35db3a02-5295-7ff3-aca8-38f030f04457",
            uuid_version::<7>(&mut g)
        );
    }

    fn assert_valid_uuid(s: &str, version: char) {
        assert_eq!(36, s.len());
        for (idx, ch) in s.chars().enumerate() {
            match idx {
                8 | 13 | 18 | 23 => assert_eq!('-', ch),
                14 => assert_eq!(version, ch),
                19 => assert!(matches!(ch, '8' | '9' | 'a' | 'b')),
                _ => assert!(ch.is_ascii_hexdigit()),
            }
        }
    }

    #[test]
    fn combinator_uuid_is_valid() {
        let mut g = Guacamole::default();
        assert_valid_uuid(&uuid(&mut g), '4');
        assert_valid_uuid(&uuid_version::<7>(&mut g), '7');
    }

    #[test]
    #[should_panic]
    fn combinator_uuid_invalid_version_panics() {
        let mut g = Guacamole::default();
        let _ = uuid_version::<1>(&mut g);
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

    #[test]
    #[should_panic]
    fn to_charset_empty_panics() {
        let _ = to_charset("");
    }

    #[test]
    #[should_panic]
    fn to_charset_too_large_panics() {
        let chars: String = (0u32..257)
            .map(|x| char::from_u32('A' as u32 + x).unwrap_or('\u{FFFD}'))
            .collect();
        let _ = to_charset(&chars);
    }
}
