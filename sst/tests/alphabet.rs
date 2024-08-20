extern crate sst;

#[macro_export]
macro_rules! alphabet_tests {
    ($($name:ident: $alphabet:expr,)*) => {
    $(
        #[cfg(test)]
        mod $name {
            use sst::{Cursor, KeyValueRef};

            #[test]
            fn step_the_alphabet_forward() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::step_the_alphabet_forward"));
                cursor.seek_to_first().unwrap();
                assert_eq!(None, cursor.key_value());
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // D-W
                for _ in 0..20 {
                    cursor.next().unwrap();
                }
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Last
                cursor.next().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
            }

            #[test]
            fn step_the_alphabet_reverse() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::step_the_alphabet_reverse"));
                cursor.seek_to_last().unwrap();
                assert_eq!(None, cursor.key_value());
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // W-D
                for _ in 0..20 {
                    cursor.prev().unwrap();
                }
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Last
                cursor.prev().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
            }

            #[test]
            fn seek_to_first() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::seek_to_first"));
                cursor.seek_to_first().unwrap();
                assert_eq!(None, cursor.key_value());
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn seek_to_last() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::seek_to_last"));
                cursor.seek_to_last().unwrap();
                assert_eq!(None, cursor.key_value());
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn seek_to_at() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::seek_to_at"));
                cursor.seek("@".as_bytes()).unwrap();
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn seek_to_z() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::seek_to_z"));
                cursor.seek("Z".as_bytes()).unwrap();
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Last
                cursor.next().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
            }

            #[test]
            fn two_steps_forward_one_step_reverse() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::two_steps_forward_one_step_reverse"));
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // D-W
                for _ in 0..21 {
                    cursor.next().unwrap();
                    cursor.next().unwrap();
                    cursor.prev().unwrap();
                }
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Last
                cursor.next().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Last
                cursor.next().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
                // Last
                cursor.next().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn two_steps_reverse_one_step_forward() {
                let mut cursor = $alphabet(&(stringify!($name).to_string() + "::two_steps_reverse_one_step_forward"));
                cursor.seek_to_last().unwrap();
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValueRef {
                    key: "Z".as_bytes(),
                    timestamp: 0,
                    value: Some("z".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // X
                let exp = KeyValueRef {
                    key: "X".as_bytes(),
                    timestamp: 0,
                    value: Some("x".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValueRef {
                    key: "Y".as_bytes(),
                    timestamp: 0,
                    value: Some("y".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // W-D
                for _ in 0..21 {
                    cursor.prev().unwrap();
                    cursor.prev().unwrap();
                    cursor.next().unwrap();
                }
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // C
                let exp = KeyValueRef {
                    key: "C".as_bytes(),
                    timestamp: 0,
                    value: Some("c".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValueRef {
                    key: "B".as_bytes(),
                    timestamp: 0,
                    value: Some("b".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.prev().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // First
                cursor.prev().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
                // First
                cursor.prev().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
                // First
                cursor.prev().unwrap();
                let got = cursor.key_value();
                assert_eq!(None, got);
                // A
                let exp = KeyValueRef {
                    key: "A".as_bytes(),
                    timestamp: 0,
                    value: Some("a".as_bytes()),
                };
                cursor.next().unwrap();
                let got = cursor.key_value().unwrap();
                assert_eq!(exp, got);
            }
        }
    )*
    }
}
