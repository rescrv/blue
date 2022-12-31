extern crate lp;

#[macro_export]
macro_rules! alphabet_tests {
    ($($name:ident: $alphabet:expr,)*) => {
    $(
        #[cfg(test)]
        mod $name {
            use lp::{Cursor, KeyValueRef};

            #[test]
            fn step_the_alphabet_forward() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::step_the_alphabet_forward"));
                iter.seek_to_first().unwrap();
                assert_eq!(None, iter.value());
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // D-W
                for _ in 0..20 {
                    iter.next().unwrap();
                }
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Last
                iter.next().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
            }

            #[test]
            fn step_the_alphabet_reverse() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::step_the_alphabet_reverse"));
                iter.seek_to_last().unwrap();
                assert_eq!(None, iter.value());
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // W-D
                for _ in 0..20 {
                    iter.prev().unwrap();
                }
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Last
                iter.prev().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
            }

            #[test]
            fn seek_to_first() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::seek_to_first"));
                iter.seek_to_first().unwrap();
                assert_eq!(None, iter.value());
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn seek_to_last() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::seek_to_last"));
                iter.seek_to_last().unwrap();
                assert_eq!(None, iter.value());
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn seek_to_at() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::seek_to_at"));
                iter.seek("@".as_bytes(), 0).unwrap();
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn seek_to_z() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::seek_to_z"));
                iter.seek("Z".as_bytes(), 0).unwrap();
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Last
                iter.next().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
            }

            #[test]
            fn two_steps_forward_one_step_reverse() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::two_steps_forward_one_step_reverse"));
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // D-W
                for _ in 0..21 {
                    iter.next().unwrap();
                    iter.next().unwrap();
                    iter.prev().unwrap();
                }
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Last
                iter.next().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Last
                iter.next().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
                // Last
                iter.next().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn two_steps_reverse_one_step_forward() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::two_steps_reverse_one_step_forward"));
                iter.seek_to_last().unwrap();
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // W-D
                for _ in 0..21 {
                    iter.prev().unwrap();
                    iter.prev().unwrap();
                    iter.next().unwrap();
                }
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.prev().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // First
                iter.prev().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
                // First
                iter.prev().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
                // First
                iter.prev().unwrap();
                let got = iter.value();
                assert_eq!(None, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                iter.next().unwrap();
                let got = iter.value().unwrap();
                assert_eq!(exp, got);
            }
        }
    )*
    }
}
