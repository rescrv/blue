extern crate scrunch;

use scrunch::reference::ReferenceIndex;
use scrunch::Index;

pub const README: &str = "README";
pub const LICENSE: &str = "LICENSE";

pub const A_TALE_OF_TWO_CITIES: &str = "A.Tale.of.Two.Cities.txt";
pub const THE_ADVENTURES_OF_TOM_SAWYER: &str = "The.Adventures.of.Tom.Sawyer.txt";
pub const THE_COUNT_OF_MONTE_CRISTO: &str = "The.Count.of.Monte.Cristo.txt";
pub const THE_ADVENTURES_OF_SHERLOCK_HOLMES: &str = "The.Adventures.of.Sherlock.Holmes.txt";
pub const DRACULA: &str = "Dracula.txt";
pub const THE_ILIAD: &str = "The.Iliad.txt";

// Common words in the gutenberg texts, arranged in rough order of frequency.
pub const COMMON_WORDS: &[&str] = &[
    "the", "and", "to", "of", "a", "in", "he", "his", "you", "that",
];

pub fn load_gutenberg(which: &str) -> Vec<char> {
    use std::fs::read_to_string;
    use std::path::PathBuf;
    let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    file.push("resources");
    file.push("gutenberg");
    file.push(which);
    read_to_string(file).unwrap().chars().into_iter().collect()
}

pub fn check_gutenberg_length<I>(reference: &ReferenceIndex, index: &I)
where
    I: Index<Item = char>,
{
    assert_eq!(reference.length(), index.length());
}

pub fn check_gutenberg_extract<I>(reference: &ReferenceIndex, index: &I)
where
    I: Index<Item = char>,
{
    for offset in 0..reference.length() - 5 {
        for count in 1..5 {
            let expected: Vec<char> = reference.extract(offset, count).unwrap().collect();
            let returned: Vec<char> = index.extract(offset, count).unwrap().collect();
            assert_eq!(expected, returned);
        }
    }
}

pub fn check_gutenberg_search<I>(reference: &ReferenceIndex, index: &I)
where
    I: Index<Item = char>,
{
    for word in COMMON_WORDS {
        let needle: Vec<char> = word.chars().into_iter().collect();
        let mut expected: Vec<usize> = reference.search(&needle).collect();
        let mut returned: Vec<usize> = index.search(&needle).collect();
        expected.sort_unstable();
        returned.sort_unstable();
        assert_eq!(expected, returned);
    }
}

pub fn check_gutenberg_count<I>(reference: &ReferenceIndex, index: &I)
where
    I: Index<Item = char>,
{
    for word in COMMON_WORDS {
        let needle: Vec<char> = word.chars().into_iter().collect();
        let expected = reference.count(&needle);
        let returned = index.count(&needle);
        assert_eq!(expected, returned);
    }
}

pub fn check_gutenberg<I>(which: &str, f: fn(&[char]) -> I)
where
    I: Index<Item = char>,
{
    let s = load_gutenberg(which);
    let index = f(&s);
    let reference = ReferenceIndex::new(&s);
    check_gutenberg_length(&reference, &index);
    check_gutenberg_extract(&reference, &index);
    check_gutenberg_search(&reference, &index);
    check_gutenberg_count(&reference, &index);
}

#[macro_export]
macro_rules! gutenberg_tests {
    ($($name:ident: $value:expr,)*) => {
    $(
        mod $name {
            use super::common::check_gutenberg;

            #[test]
            fn readme() {
                check_gutenberg(super::common::README, $value);
            }

            #[test]
            fn license() {
                check_gutenberg(super::common::LICENSE, $value);
            }

            #[test]
            fn a_tale_of_two_cities() {
                check_gutenberg(super::common::A_TALE_OF_TWO_CITIES, $value);
            }

            #[test]
            fn the_adventures_of_tom_sawyer() {
                check_gutenberg(super::common::THE_ADVENTURES_OF_TOM_SAWYER, $value);
            }

            #[test]
            fn the_count_of_monte_cristo() {
                check_gutenberg(super::common::THE_COUNT_OF_MONTE_CRISTO, $value);
            }

            #[test]
            fn the_adventures_of_sherlock_holmes() {
                check_gutenberg(super::common::THE_ADVENTURES_OF_SHERLOCK_HOLMES, $value);
            }

            #[test]
            fn dracula() {
                check_gutenberg(super::common::DRACULA, $value);
            }

            #[test]
            fn the_iliad() {
                check_gutenberg(super::common::THE_ILIAD, $value);
            }
        }
    )*
    }
}
