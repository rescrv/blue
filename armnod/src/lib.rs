use rand::RngCore;

use guacamole::Guacamole;
use guacamole::Zipf;

//////////////////////////////////////////// SeedChoice ////////////////////////////////////////////

pub enum SeedChoice {
    StopIterating,
    SkipSeeding,
    Seed(u64),
}

//////////////////////////////////////////// SeedChooser ///////////////////////////////////////////

pub trait SeedChooser {
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice;
}

//////////////////////////////////////// RandomStringChooser ///////////////////////////////////////

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

pub struct SetStringChooser {
    cardinality: u64,
}

impl SetStringChooser {
    pub fn new(cardinality: u64) -> Self {
        Self {
            cardinality,
        }
    }
}

impl SeedChooser for SetStringChooser {
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice {
        let seed = guac.next_u64();
        let index = seed % self.cardinality;
        SeedChoice::Seed(index * SET_SPREADER)
    }
}

/////////////////////////////////////// SetStringChooserOnce ///////////////////////////////////////

pub struct SetStringChooserOnce {
    start: u64,
    limit: u64,
}

impl SetStringChooserOnce {
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

pub struct SetStringChooserZipf {
    zipf: Zipf,
}

impl SetStringChooserZipf {
    pub fn from_alpha(n: u64, alpha: f64) -> Self {
        Self {
            zipf: Zipf::from_alpha(n, alpha),
        }
    }

    pub fn from_theta(n: u64, theta: f64) -> Self {
        Self {
            zipf: Zipf::from_theta(n, theta),
        }
    }
}

impl SeedChooser for SetStringChooserZipf {
    fn which_seed(&mut self, guac: &mut Guacamole) -> SeedChoice {
        SeedChoice::Seed(self.zipf.next(guac))
    }
}

/////////////////////////////////////////// LengthChooser //////////////////////////////////////////

pub trait LengthChooser {
    fn how_long(&mut self, guac: &mut Guacamole) -> u32;
}

/////////////////////////////////////// ConstantLengthChooser //////////////////////////////////////

pub struct ConstantLengthChooser {
    length: u32,
}

impl ConstantLengthChooser {
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

pub struct UniformLengthChooser {
    min_length: u32,
    max_length: u32,
}

impl UniformLengthChooser {
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

pub trait CharacterChooser {
    fn which_char(&mut self, guac: &mut Guacamole) -> char;
}

////////////////////////////////////////// CharSetChooser //////////////////////////////////////////

pub struct CharSetChooser {
    chars: [char; 256]
}

impl CharSetChooser {
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
}

////////////////////////////////////////////// ARMNOD //////////////////////////////////////////////

/// ARMNOD is an anagram of RANDOM
pub struct ARMNOD {
    pub string: Box<dyn SeedChooser>,
    pub length: Box<dyn LengthChooser>,
    pub characters: Box<dyn CharacterChooser>,
}

impl ARMNOD {
    pub fn random(length: u32) -> ARMNOD {
        let string = Box::new(RandomStringChooser::default());
        let length = Box::new(ConstantLengthChooser::new(length));
        let characters = Box::new(CharSetChooser::new(CHAR_SET_DEFAULT));
        ARMNOD {
            string,
            length,
            characters,
        }
    }

    pub fn choose(&mut self, guac: &mut Guacamole) -> Option<String> {
        match self.string.which_seed(guac) {
            SeedChoice::Seed(seed) => { self.choose_seeded(&mut Guacamole::new(seed)) },
            SeedChoice::SkipSeeding => { self.choose_seeded(guac) },
            SeedChoice::StopIterating => { None },
        }
    }

    fn choose_seeded(&mut self, guac: &mut Guacamole) -> Option<String> {
        let length = self.length.how_long(guac) as usize;
        let mut string = String::with_capacity(length);
        for _ in 0..length {
            string.push(self.characters.which_char(guac));
        }
        Some(string)
    }
}
