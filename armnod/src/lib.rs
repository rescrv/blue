#![doc = include_str!("../README.md")]

use guacamole::{FromGuacamole, Guacamole, Zipf};

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
    /// Uses the provided guacamole to return a seed choice.
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice;
}

//////////////////////////////////////// RandomStringChooser ///////////////////////////////////////

/// [RandomStringChooser] skips seeding entirely.  This saves on CPU for when all that's needed are
/// pseudo-random strings.
#[derive(Default)]
pub struct RandomStringChooser {}

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
        Self { cardinality }
    }
}

impl SeedChooser for SetStringChooser {
    /// Endlessly return the next seed for strings on `[0, cardinality)`.  Note that the SeedChoice
    /// will not be over the same interval, but over the complete u64.
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice {
        let seed = u64::from_guacamole(&mut (), guac);
        let index = seed % self.cardinality;
        SeedChoice::Seed(index.wrapping_mul(SET_SPREADER))
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
        Self { start, limit }
    }
}

impl SeedChooser for SetStringChooserOnce {
    fn which_seed(&mut self, _: &mut Guacamole) -> SeedChoice {
        let index = self.start;
        if index < self.limit {
            self.start += 1;
            SeedChoice::Seed(index.wrapping_mul(SET_SPREADER))
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
    /// Configure the Zipf distribution to be `(0, n)` with `param`.
    pub fn from_param(n: u64, param: f64) -> Self {
        Self {
            zipf: Zipf::from_param(n, param),
        }
    }

    /// Configure the Zipf distribution to be `[0, n)` with `alpha`.
    #[deprecated(since = "0.12.0", note = "Use `from_param` instead")]
    pub fn from_alpha(n: u64, alpha: f64) -> Self {
        Self {
            #[allow(deprecated)]
            zipf: Zipf::from_alpha(n, alpha),
        }
    }

    /// Configure the Zipf distribution to be `[0, n)` with `theta`.
    #[deprecated(since = "0.12.0", note = "Use `from_param` instead")]
    pub fn from_theta(n: u64, theta: f64) -> Self {
        Self {
            #[allow(deprecated)]
            zipf: Zipf::from_theta(n, theta),
        }
    }
}

impl SeedChooser for SetStringChooserZipf {
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice {
        SeedChoice::Seed(self.zipf.next(guac).wrapping_mul(SET_SPREADER))
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
        Self { length }
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
        let offset = u32::from_guacamole(&mut (), guac) % range;
        assert!(self.min_length + offset <= self.max_length);
        self.min_length + offset
    }
}

///////////////////////////////////////// CharacterChooser /////////////////////////////////////////

/// The lower character set includes lower-case ASCII alphabets.
pub const CHAR_SET_LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
/// The upper character set includes upper-case ASCII alphabets.
pub const CHAR_SET_UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The alph character set includes lower- and upper-case ASCII alphabets.
pub const CHAR_SET_ALPHA: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
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
    chars: [char; 256],
}

impl CharSetChooser {
    /// Create a new [CharSetChooser] that will select characters from `s` uniformly at random.
    pub fn new(s: &str) -> Self {
        let s: Vec<char> = s.chars().collect();
        assert!(s.len() < 96);
        let mut table: [char; 256] = ['A'; 256];

        for (i, x) in table.iter_mut().enumerate() {
            let d: f64 = (i as f64) / 256.0 * s.len() as f64;
            let d: usize = d as usize;
            assert!(d < s.len());
            *x = s[d];
        }

        Self { chars: table }
    }
}

impl CharacterChooser for CharSetChooser {
    fn which_char(&mut self, guac: &mut Guacamole) -> char {
        let mut byte = [0u8; 1];
        guac.generate(&mut byte);
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

////////////////////////////////////////////// Armnod //////////////////////////////////////////////

/// Armnod is an anagram of Random
pub struct Armnod {
    /// How to select the seed for each string.
    pub string: Box<dyn SeedChooser>,
    /// How to select the length of each string.
    pub length: Box<dyn LengthChooser>,
    /// The characters to pick for strings.
    pub characters: Box<dyn CharacterChooser>,
    /// The buffer used for returning strings.
    pub buffer: Vec<u8>,
}

impl Armnod {
    /// Create a new [Armnod] for strings of `length` with the default character set.
    pub fn random(length: u32) -> Self {
        let string = Box::<RandomStringChooser>::default();
        let length = Box::new(ConstantLengthChooser::new(length));
        let characters = Box::new(CharSetChooser::new(CHAR_SET_DEFAULT));
        Self {
            string,
            length,
            characters,
            buffer: Vec::new(),
        }
    }

    /// Choose the next string from the provided guacamole.
    pub fn choose(&mut self, guac: &mut Guacamole) -> Option<String> {
        match self.string.which_seed(guac) {
            SeedChoice::Seed(seed) => self.choose_seeded(&mut Guacamole::new(seed)),
            SeedChoice::SkipSeeding => self.choose_seeded(guac),
            SeedChoice::StopIterating => None,
        }
    }

    fn choose_seeded(&mut self, guac: &mut Guacamole) -> Option<String> {
        let length = self.length.how_long(guac) as usize;
        self.buffer.resize(length, 0);
        guac.generate(&mut self.buffer);
        let string = self.characters.whole_string(&mut self.buffer);
        Some(string)
    }
}

/////////////////////////////////////////// Command Line ///////////////////////////////////////////

/// Options for constructing an Armnod instance.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct ArmnodOptions {
    /// The method of choosing strings.
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            required,
            "Method of choosing strings (random, set, set-once, set-zipf).",
            "METHOD"
        )
    )]
    pub chooser_mode: String,
    /// The size of the set for set-based chooser modes.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Cardinality for set-based modes.", "N")
    )]
    pub cardinality: Option<u64>,
    /// The first element to load in set-once mode.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "First set element to load in set-once mode.", "ELEM")
    )]
    pub set_once_begin: Option<u64>,
    /// The last element to load in set-once mode.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Last set element to load in set-once mode.", "ELEM")
    )]
    pub set_once_end: Option<u64>,
    /// The zipf alpha parameter.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Alpha value for the zipf distribution.", "ALPHA")
    )]
    pub zipf_alpha: Option<f64>,
    /// The zipf theta parameter.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Theta value for the zipf distribution.", "THETA")
    )]
    pub zipf_theta: Option<f64>,
    /// The method chosen for picking string length.  One of "constant", "uniform".
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Method of choosing length.", "LENGTH")
    )]
    pub length_mode: Option<String>,
    /// The constant length of strings when using constant length modes.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Generate strings of this constant length.", "LENGTH")
    )]
    pub length: Option<u32>,
    /// The average length of strings when using varied length modes.
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Generate strings at this average length for varied length modes.",
            "AVG"
        )
    )]
    pub avg_length: Option<u32>,
    /// The minimum length of strings when using varied length modes.
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Generate strings at least this length for varied length modes.",
            "MIN"
        )
    )]
    pub min_length: Option<u32>,
    /// The maximum length of strings when using varied length modes.
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            optional,
            "Generate strings at most this length for varied length modes.",
            "MAX"
        )
    )]
    pub max_length: Option<u32>,
    /// The charset to use.
    #[cfg_attr(feature = "command_line", arrrg(optional, "Use this character set.  Provided are lower, upper, alpha, digit, alnum, punct, hex, and default.", "CHARSET"))]
    pub charset: Option<String>,
}

fn random_chooser() -> Box<dyn SeedChooser> {
    Box::<RandomStringChooser>::default()
}

fn set_chooser(cardinality: u64) -> Box<dyn SeedChooser> {
    Box::new(SetStringChooser::new(cardinality))
}

fn set_chooser_once(begin: u64, end: u64) -> Box<dyn SeedChooser> {
    Box::new(SetStringChooserOnce::new(begin, end))
}

fn set_chooser_zipf_theta(cardinality: u64, theta: f64) -> Box<dyn SeedChooser> {
    #[allow(deprecated)]
    Box::new(SetStringChooserZipf::from_theta(cardinality, theta))
}

fn set_chooser_zipf_alpha(cardinality: u64, alpha: f64) -> Box<dyn SeedChooser> {
    #[allow(deprecated)]
    Box::new(SetStringChooserZipf::from_alpha(cardinality, alpha))
}

fn constant_length_chooser(length: u32) -> Box<dyn LengthChooser> {
    Box::new(ConstantLengthChooser::new(length))
}

fn uniform_length_chooser(min_length: u32, max_length: u32) -> Box<dyn LengthChooser> {
    Box::new(UniformLengthChooser::new(min_length, max_length))
}

impl ArmnodOptions {
    /// Try to parse the armnod options.
    pub fn try_parse(self) -> Result<Armnod, String> {
        self.try_parse_sharded(0, 1)
    }

    /// Try to parse the armnod options for shard `index` where there are `total` shards.
    pub fn try_parse_sharded(self, index: u64, total: u64) -> Result<Armnod, String> {
        // string chooser
        let string_chooser = if self.chooser_mode == "random" {
            random_chooser()
        } else if self.chooser_mode == "set" {
            set_chooser(self.cardinality.unwrap_or(1_000_000))
        } else if self.chooser_mode == "set-once" {
            let cardinality = self.cardinality.unwrap_or(1_000_000);
            let step = cardinality / total;
            let thresh = cardinality % total;
            let mut begin = 0;
            for i in 0..index {
                begin += step;
                if i < thresh {
                    begin += 1;
                }
            }
            let end = if index < thresh {
                begin + step + 1
            } else {
                begin + step
            };
            let set_once_begin = self.set_once_begin.unwrap_or(begin);
            let set_once_end = self.set_once_end.unwrap_or(end);
            if set_once_begin > set_once_end {
                return Err(format!(
                    "--set-once-begin must be <= --set-once-end: {} > {}",
                    set_once_begin, set_once_end
                ));
            }
            set_chooser_once(set_once_begin, set_once_end)
        } else if self.chooser_mode == "set-zipf" {
            let cardinality = self.cardinality.unwrap_or(1_000_000);
            if let Some(zipf_theta) = self.zipf_theta {
                set_chooser_zipf_theta(cardinality, zipf_theta)
            } else if let Some(zipf_alpha) = self.zipf_alpha {
                set_chooser_zipf_alpha(cardinality, zipf_alpha)
            } else {
                set_chooser_zipf_theta(cardinality, 0.99)
            }
        } else {
            return Err(format!("unknown chooser mode: {}", self.chooser_mode));
        };
        // length chooser
        let length_mode = self.length_mode.unwrap_or("constant".to_string());
        let length_chooser = if length_mode == "constant" {
            if self.min_length.is_some() {
                return Err(
                    "--string-min-length not supported for --length-mode=constant".to_string(),
                );
            }
            if self.max_length.is_some() {
                return Err(
                    "--string-max-length not supported for --length-mode=constant".to_string(),
                );
            }
            if self.avg_length.is_some() {
                return Err(
                    "--string-avg-length not supported for --length-mode=constant".to_string(),
                );
            }
            constant_length_chooser(self.length.unwrap_or(8))
        } else if length_mode == "uniform" {
            let min_length: u32 = self.min_length.unwrap_or(8);
            let max_length: u32 = self.max_length.unwrap_or(min_length + 8);
            if min_length > max_length {
                return Err(format!(
                    "--string-min-length must be <= --string-max-length: {} > {}",
                    min_length, max_length
                ));
            }
            if self.length.is_some() {
                return Err("--string-length not supported for --length-mode=uniform".to_string());
            }
            if self.avg_length.is_some() {
                return Err(
                    "--string-avg-length not supported for --length-mode=uniform".to_string(),
                );
            }
            uniform_length_chooser(min_length, max_length)
        } else {
            return Err(format!("unknown length mode: {}", length_mode));
        };
        // alphabet to use
        let charset = self.charset.unwrap_or("default".to_string());
        let characters = if charset == "default" {
            CharSetChooser::new(CHAR_SET_DEFAULT)
        } else if charset == "lower" {
            CharSetChooser::new(CHAR_SET_LOWER)
        } else if charset == "upper" {
            CharSetChooser::new(CHAR_SET_UPPER)
        } else if charset == "alpha" {
            CharSetChooser::new(CHAR_SET_ALPHA)
        } else if charset == "digit" {
            CharSetChooser::new(CHAR_SET_DIGIT)
        } else if charset == "alnum" {
            CharSetChooser::new(CHAR_SET_ALNUM)
        } else if charset == "punct" {
            CharSetChooser::new(CHAR_SET_PUNCT)
        } else if charset == "hex" {
            CharSetChooser::new(CHAR_SET_HEX)
        } else {
            return Err(format!("unknown character set: {}", charset));
        };
        let characters: Box<dyn CharacterChooser> = Box::new(characters);
        // generate strings
        Ok(Armnod {
            string: string_chooser,
            length: length_chooser,
            characters,
            buffer: Vec::new(),
        })
    }
}

impl Default for ArmnodOptions {
    fn default() -> Self {
        Self {
            chooser_mode: "random".to_string(),
            cardinality: None,
            set_once_begin: None,
            set_once_end: None,
            zipf_alpha: None,
            zipf_theta: None,
            length_mode: None,
            length: None,
            avg_length: None,
            min_length: None,
            max_length: None,
            charset: None,
        }
    }
}

impl Eq for ArmnodOptions {}
