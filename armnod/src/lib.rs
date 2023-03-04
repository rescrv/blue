//! Armnod is an anagram for "random"; Armnod is a library for working with random strings.
//!
//! Each [ARMNOD] consists of a [SeedChooser], [LengthChooser], and [CharacterChooser] which
//! compose a set of random strings.  The SeedChooser picks the element of the set.  It may say to
//! stop iterating (enough items have been chosen), it may say to seek to a particular offset in
//! another guacamole generator (there's a finite number of seeds), or it may say to not seek at
//! all (an "infinite" number of strings are possible).
//!
//! The [SeedChooser] and [LengthChooser] both pull from a [guacamole::Guacamole] stream to
//! generate the seed and a u32 for the string's length.  It's easy to see that when the `guac` is
//! positioned at the same point in the stream, the seed and length will be the same.
//!
//! [CharacterChooser] pulls bytes from the string and maps them to characters to create a string.
//! Essentially mapping the binary data to ASCII data.  UTF-8 marginally supported.

use rand::RngCore;

use guacamole::Guacamole;
use guacamole::Zipf;

//////////////////////////////////////////// SeedChoice ////////////////////////////////////////////

/// [SeedChoice] chooses the string to be generated from one of three possibilities.  The first
/// could be to not generate anything and stop iterating.  This is useful, e.g., when the
/// [SeedChoice] generates each string exactly once for purposes of loading data.  The second
/// choice is to skip seeding.  This will avoid a seek at the lower level random number generator,
/// but it means the data generated will truly span the domain of [Guacamole].
pub enum SeedChoice {
    /// Do not return a string.  Stop work instead.
    StopIterating,
    /// Use the existing guacamole for generating strings.  You get what you get.
    SkipSeeding,
    /// Seed guacamole before generating strings.  If the `u64` is drawn from a consistent set, the
    /// set of strings generated will also be consistent and of equal cardinality.
    Seed(u64),
}

//////////////////////////////////////////// SeedChooser ///////////////////////////////////////////

/// A [SeedChooser] returns a [SeedChoice] using the next value drawn from `guac`.
pub trait SeedChooser {
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice;
}

//////////////////////////////////////// RandomStringChooser ///////////////////////////////////////

/// [RandomStringChooser] skips seeding entirely.  This saves on CPU for when all that's needed are
/// pseudo-random strings.
#[derive(Default)]
pub struct RandomStringChooser {
}

impl SeedChooser for RandomStringChooser {
    fn which_seed(&mut self, _: &mut Guacamole) -> SeedChoice {
        SeedChoice::SkipSeeding
    }
}

///////////////////////////////////////// SetStringChooser /////////////////////////////////////////

const SET_SPREADER: u64 = 4294967291;

/// Choose strings from a set of strings with uniform probability.
pub struct SetStringChooser {
    cardinality: u64,
}

impl SetStringChooser {
    /// Create a new [SetStringChooser] with `cardinality`.  Strings are on `[0, cardinality)` and
    /// will be generated using two levels of [Guacamole].
    pub fn new(cardinality: u64) -> Self {
        Self {
            cardinality,
        }
    }
}

impl SeedChooser for SetStringChooser {
    /// Endlessly return the next seed for strings on `[0, cardinality)`.  Note that the SeedChoice
    /// will not be over the same interval, but over the complete u64.
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice {
        let seed = guac.next_u64();
        let index = seed % self.cardinality;
        SeedChoice::Seed(index * SET_SPREADER)
    }
}

/////////////////////////////////////// SetStringChooserOnce ///////////////////////////////////////

/// Return a range of strings over `[start, limit)`.
pub struct SetStringChooserOnce {
    start: u64,
    limit: u64,
}

impl SetStringChooserOnce {
    /// Create a new [SetStringChooserOnce].  The generated seeds will be only for the strings
    /// between [start, limit), which is assumed to be a chunk of `[0, n)`.
    pub fn new(start: u64, limit: u64) -> Self {
        assert!(start <= limit);
        Self {
            start,
            limit,
        }
    }
}

impl SeedChooser for SetStringChooserOnce {
    fn which_seed(&mut self, _: &mut Guacamole) -> SeedChoice {
        let index = self.start;
        if index < self.limit {
            self.start += 1;
            SeedChoice::Seed(index * SET_SPREADER)
        } else {
            SeedChoice::StopIterating
        }
    }
}

/////////////////////////////////////// SetStringChooserZipf ///////////////////////////////////////

/// Draw values according to a Zipf distribution.
pub struct SetStringChooserZipf {
    zipf: Zipf,
}

impl SetStringChooserZipf {
    /// Configure the Zipf distribution to be `[0, n)` with `alpha`.
    pub fn from_alpha(n: u64, alpha: f64) -> Self {
        Self {
            zipf: Zipf::from_alpha(n, alpha),
        }
    }

    /// Configure the Zipf distribution to be `[0, n)` with `theta`.
    pub fn from_theta(n: u64, theta: f64) -> Self {
        Self {
            zipf: Zipf::from_theta(n, theta),
        }
    }
}

impl SeedChooser for SetStringChooserZipf {
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice {
        SeedChoice::Seed(self.zipf.next(guac) * SET_SPREADER)
    }
}

/////////////////////////////////////////// LengthChooser //////////////////////////////////////////

/// Given [Guacamole], generate a string length for the next string.
pub trait LengthChooser {
    /// Use the provided guacamole as the sole source of randomness to generate a new `u32` string
    /// length.
    fn how_long(&mut self, guac: &mut Guacamole) -> u32;
}

/////////////////////////////////////// ConstantLengthChooser //////////////////////////////////////

/// [ConstantLengthChooser] generates strings of uniform length.
pub struct ConstantLengthChooser {
    length: u32,
}

impl ConstantLengthChooser {
    /// Create a new [ConstantLengthChooser] that will generate strings of `length` characters.
    pub fn new(length: u32) -> Self {
        Self {
            length,
        }
    }
}

impl LengthChooser for ConstantLengthChooser {
    fn how_long(&mut self, _: &mut Guacamole) -> u32 {
        self.length
    }
}

/////////////////////////////////////// UniformLengthChooser ///////////////////////////////////////

/// [UniformLengthChooser] generates strings with a uniform distribution between a minimum and
/// maximum length.
pub struct UniformLengthChooser {
    min_length: u32,
    max_length: u32,
}

impl UniformLengthChooser {
    /// Create a new [UniformLengthChooser] with `min_length` and `max_length`.
    pub fn new(min_length: u32, max_length: u32) -> Self {
        assert!(min_length <= max_length);
        Self {
            min_length,
            max_length,
        }
    }
}

impl LengthChooser for UniformLengthChooser {
    fn how_long(&mut self, guac: &mut Guacamole) -> u32 {
        let range = self.max_length - self.min_length + 1;
        let offset = guac.next_u32() % range;
        assert!(self.min_length + offset <= self.max_length);
        self.min_length + offset
    }
}

///////////////////////////////////////// CharacterChooser /////////////////////////////////////////

pub const CHAR_SET_LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
pub const CHAR_SET_UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const CHAR_SET_ALPHA: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const CHAR_SET_DIGIT: &str = "0123456789";
pub const CHAR_SET_ALNUM: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
pub const CHAR_SET_PUNCT: &str = "!\"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~";
pub const CHAR_SET_HEX: &str = "0123456789abcdef";
pub const CHAR_SET_DEFAULT: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!\"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~";

/// Generate strings of a given alphabet using the provided randomness.
pub trait CharacterChooser {
    /// Generate a single char according to the distribution, using `guac`.
    fn which_char(&mut self, guac: &mut Guacamole) -> char;
    /// Generate a string equal in length to `bytes`, using each character of `bytes` as a random
    /// value to pick another character.  NOTE(rescrv):  This implies the [CharacterChooser] API
    /// only supports 256 distinct characters at a time, and I'm OK with that.
    fn whole_string(&mut self, bytes: &mut [u8]) -> String;
}

////////////////////////////////////////// CharSetChooser //////////////////////////////////////////

/// Choose characters from alphabets of up to 96 characters.
pub struct CharSetChooser {
    chars: [char; 256]
}

impl CharSetChooser {
    /// Create a new [CharSetChooser] that will select characters from `s` uniformly at random.
    pub fn new(s: &str) -> Self {
        let s: Vec<char> = s.chars().collect();
        assert!(s.len() < 96);
        let mut table: [char; 256] = ['A'; 256];

        for i in 0..256 {
            let d: f64 = (i as f64) / 256.0 * s.len() as f64;
            let d: usize = d as usize;
            assert!(d < s.len());
            table[i] = s[d];
        }

        Self {
            chars: table,
        }
    }
}

impl CharacterChooser for CharSetChooser {
    fn which_char(&mut self, guac: &mut Guacamole) -> char {
        let mut byte = [0u8; 1];
        guac.fill_bytes(&mut byte);
        self.chars[byte[0] as usize]
    }

    fn whole_string(&mut self, bytes: &mut [u8]) -> String {
        let mut string = String::with_capacity(bytes.len());
        for b in bytes.iter() {
            string.push(self.chars[*b as usize]);
        }
        string
    }
}

////////////////////////////////////////////// ARMNOD //////////////////////////////////////////////

/// ARMNOD is an anagram of RANDOM
pub struct ARMNOD {
    pub string: Box<dyn SeedChooser>,
    pub length: Box<dyn LengthChooser>,
    pub characters: Box<dyn CharacterChooser>,
    pub buffer: Vec<u8>,
}

impl ARMNOD {
    /// Create a new [ARMNOD] for strings of `length` with the default character set.
    pub fn random(length: u32) -> ARMNOD {
        let string = Box::<RandomStringChooser>::default();
        let length = Box::new(ConstantLengthChooser::new(length));
        let characters = Box::new(CharSetChooser::new(CHAR_SET_DEFAULT));
        ARMNOD {
            string,
            length,
            characters,
            buffer: Vec::new(),
        }
    }

    /// Choose the next string from the provided guacamole.
    pub fn choose(&mut self, guac: &mut Guacamole) -> Option<String> {
        match self.string.which_seed(guac) {
            SeedChoice::Seed(seed) => { self.choose_seeded(&mut Guacamole::new(seed)) },
            SeedChoice::SkipSeeding => { self.choose_seeded(guac) },
            SeedChoice::StopIterating => { None },
        }
    }

    fn choose_seeded(&mut self, guac: &mut Guacamole) -> Option<String> {
        let length = self.length.how_long(guac) as usize;
        self.buffer.resize(length, 0);
        guac.fill_bytes(&mut self.buffer);
        let string = self.characters.whole_string(&mut self.buffer);
        Some(string)
    }
}
