extern crate scrunch;

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

#[macro_export]
macro_rules! gutenberg_texts {
    ($to_call:ident) => {
        #[test]
        fn readme() {
            $to_call($crate::common::README);
        }

        #[test]
        fn license() {
            $to_call($crate::common::LICENSE);
        }

        #[test]
        fn a_tale_of_two_cities() {
            $to_call($crate::common::A_TALE_OF_TWO_CITIES);
        }

        #[test]
        fn the_adventures_of_tom_sawyer() {
            $to_call($crate::common::THE_ADVENTURES_OF_TOM_SAWYER);
        }

        #[test]
        fn the_count_of_monte_cristo() {
            $to_call($crate::common::THE_COUNT_OF_MONTE_CRISTO);
        }

        #[test]
        fn the_adventures_of_sherlock_holmes() {
            $to_call($crate::common::THE_ADVENTURES_OF_SHERLOCK_HOLMES);
        }

        #[test]
        fn dracula() {
            $to_call($crate::common::DRACULA);
        }

        #[test]
        fn the_iliad() {
            $to_call($crate::common::THE_ILIAD);
        }
    };
}

pub fn load_gutenberg(which: &str) -> (Vec<u32>, Vec<usize>) {
    use std::fs::read_to_string;
    use std::path::PathBuf;
    let mut file = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    file.push("resources");
    file.push("gutenberg");
    file.push(which);
    let text: Vec<u32> = read_to_string(file)
        .unwrap()
        .chars()
        .map(|c| c as u32)
        .into_iter()
        .collect();
    let mut record_boundaries = vec![0usize];
    for (idx, _) in text.iter().enumerate().filter(|(_, t)| **t == '\n' as u32) {
        record_boundaries.push(idx + 1);
    }
    if record_boundaries[record_boundaries.len() - 1] == text.len() {
        record_boundaries.pop();
    }
    (text, record_boundaries)
}

#[macro_export]
macro_rules! gutenberg_tests {
    ($name:ident, $document:path) => {
        mod $name {
            use buffertk::Unpackable;

            use ::scrunch::Document;

            fn construct<'a, 'b>(
                input: &str,
                buf1: &'a mut Vec<u8>,
                buf2: &'b mut Vec<u8>,
            ) -> (
                ::scrunch::ReferenceDocument,
                Box<$document>,
            ) {
                buf1.clear();
                let (text, record_boundaries) = $crate::common::load_gutenberg(input);
                let mut builder = ::scrunch::builder::Builder::new(buf1);
                ::scrunch::ReferenceDocument::construct(
                    text.clone(),
                    record_boundaries.clone(),
                    &mut builder,
                )
                .expect("should construct document");
                drop(builder);
                let reference = ::scrunch::ReferenceDocument::unpack(&buf1)
                    .expect("should parse document").0;

                buf2.clear();
                let mut builder = ::scrunch::builder::Builder::new(buf2);
                <$document>::construct(text, record_boundaries, &mut builder)
                    .expect("should construct document");
                drop(builder);
                let under_test = <$document>::unpack(buf2)
                    .expect("should parse document").0;
                (reference, Box::new(under_test))
            }

            mod len {
                use ::scrunch::Document;

                fn check_len(input: &str) {
                    let mut reference_buf = vec![];
                    let mut under_test_buf = vec![];
                    let (reference, under_test) =
                        super::construct(input, &mut reference_buf, &mut under_test_buf);

                    assert_eq!(reference.len(), under_test.len(), "foo");
                }

                $crate::gutenberg_texts! {check_len}
            }

            mod records {
                use ::scrunch::Document;

                fn check_records(input: &str) {
                    let mut reference_buf = vec![];
                    let mut under_test_buf = vec![];
                    let (reference, under_test) =
                        super::construct(input, &mut reference_buf, &mut under_test_buf);

                    assert_eq!(reference.records(), under_test.records());
                }

                $crate::gutenberg_texts! {check_records}
            }

            mod search {
                use ::scrunch::Document;

                fn check_search(input: &str) {
                    let mut reference_buf = vec![];
                    let mut under_test_buf = vec![];
                    let (reference, under_test) =
                        super::construct(input, &mut reference_buf, &mut under_test_buf);

                    for word in $crate::common::COMMON_WORDS.iter() {
                        let word: Vec<u32> = word.chars().map(|c| c as u32).collect();
                        let expected: Vec<::scrunch::TextOffset> = reference
                            .search(&word)
                            .expect("search should succeed")
                            .collect();
                        let returned: Vec<::scrunch::TextOffset> = under_test
                            .search(&word)
                            .expect("search should succeed")
                            .collect();
                        assert_eq!(expected, returned);
                    }
                }

                $crate::gutenberg_texts! {check_search}
            }

            mod count {
                use ::scrunch::Document;

                fn check_count(input: &str) {
                    let mut reference_buf = vec![];
                    let mut under_test_buf = vec![];
                    let (reference, under_test) =
                        super::construct(input, &mut reference_buf, &mut under_test_buf);

                    for word in $crate::common::COMMON_WORDS.iter() {
                        let word: Vec<u32> = word.chars().map(|c| c as u32).collect();
                        let expected: usize =
                            reference.count(&word).expect("search should succeed");
                        let returned: usize =
                            under_test.count(&word).expect("search should succeed");
                        assert_eq!(expected, returned);
                    }
                }

                $crate::gutenberg_texts! {check_count}
            }

            mod lookup {
                use ::scrunch::Document;

                fn check_lookup(input: &str) {
                    let mut reference_buf = vec![];
                    let mut under_test_buf = vec![];
                    let (reference, under_test) =
                        super::construct(input, &mut reference_buf, &mut under_test_buf);

                    for offset in 0..reference.len() {
                        let offset = ::scrunch::TextOffset(offset);
                        let expected = reference.lookup(offset).expect("lookup should succeed");
                        let returned = under_test.lookup(offset).expect("lookup should succeed");
                        assert_eq!(expected, returned);
                    }
                }

                $crate::gutenberg_texts! {check_lookup}
            }

            mod retrieve {
                use ::scrunch::Document;

                fn check_retrieve(input: &str) {
                    let mut reference_buf = vec![];
                    let mut under_test_buf = vec![];
                    let (reference, under_test) =
                        super::construct(input, &mut reference_buf, &mut under_test_buf);

                    for offset in 0..reference.records() {
                        let offset = ::scrunch::RecordOffset(offset);
                        let expected = reference.retrieve(offset).expect("retrieve should succeed");
                        let returned = under_test
                            .retrieve(offset)
                            .expect("retrieve should succeed");
                        assert_eq!(expected, returned);
                    }
                }

                $crate::gutenberg_texts! {check_retrieve}
            }

            mod offset_of {
                use ::scrunch::Document;

                fn check_offset_of(input: &str) {
                    let mut reference_buf = vec![];
                    let mut under_test_buf = vec![];
                    let (reference, under_test) =
                        super::construct(input, &mut reference_buf, &mut under_test_buf);

                    for offset in 0..reference.records() {
                        let offset = ::scrunch::RecordOffset(offset);
                        let expected = reference
                            .offset_of(offset)
                            .expect("offset_of should succeed");
                        let returned = under_test
                            .offset_of(offset)
                            .expect("offset_of should succeed");
                        assert_eq!(expected, returned);
                    }
                }

                $crate::gutenberg_texts! {check_offset_of}
            }
        }
    };
}
