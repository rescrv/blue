extern crate lp;

#[macro_export]
macro_rules! alphabet_tests {
    ($($name:ident: $alphabet:expr,)*) => {
    $(
        #[cfg(test)]
        mod $name {
            use lp::{Cursor, KeyValuePair};

            #[test]
            fn step_the_alphabet_forward() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::step_the_alphabet_forward"));
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
                // C
                let exp = KeyValuePair {
                    key: "C".into(),
                    timestamp: 0,
                    value: Some("c".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
                // D-W
                for _ in 0..20 {
                    let _got = iter.next().unwrap().unwrap();
                }
                // X
                let exp = KeyValuePair {
                    key: "X".into(),
                    timestamp: 0,
                    value: Some("x".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
                // Last
                let got = iter.next().unwrap();
                assert_eq!(None, got);
            }

            #[test]
            fn step_the_alphabet_reverse() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::step_the_alphabet_reverse"));
                iter.seek_to_last().unwrap();
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.prev().unwrap().unwrap();
                assert_eq!(exp, got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.prev().unwrap().unwrap();
                assert_eq!(exp, got);
                // X
                let exp = KeyValuePair {
                    key: "X".into(),
                    timestamp: 0,
                    value: Some("x".into()),
                };
                let got = iter.prev().unwrap().unwrap();
                assert_eq!(exp, got);
                // W-D
                for _ in 0..20 {
                    let _got = iter.prev().unwrap().unwrap();
                }
                // C
                let exp = KeyValuePair {
                    key: "C".into(),
                    timestamp: 0,
                    value: Some("c".into()),
                };
                let got = iter.prev().unwrap().unwrap();
                assert_eq!(exp, got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.prev().unwrap().unwrap();
                assert_eq!(exp, got);
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.prev().unwrap().unwrap();
                assert_eq!(exp, got);
                // Last
                let got = iter.prev().unwrap();
                assert_eq!(None, got);
            }

            #[test]
            fn seek_to_at() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::seek_to_at"));
                iter.seek("@".as_bytes(), 0).unwrap();
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
            }

            #[test]
            fn seek_to_z() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::seek_to_z"));
                iter.seek("Z".as_bytes(), 0).unwrap();
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.next().unwrap().unwrap();
                assert_eq!(exp, got);
                // Last
                let got = iter.next().unwrap();
                assert_eq!(None, got);
            }

            #[test]
            fn two_steps_forward_one_step_reverse() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::two_steps_forward_one_step_reverse"));
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // C
                let exp = KeyValuePair {
                    key: "C".into(),
                    timestamp: 0,
                    value: Some("c".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // D-W
                for _ in 0..21 {
                    iter.next().unwrap();
                    iter.next().unwrap();
                    iter.prev().unwrap();
                }
                // X
                let exp = KeyValuePair {
                    key: "X".into(),
                    timestamp: 0,
                    value: Some("x".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // X
                let exp = KeyValuePair {
                    key: "X".into(),
                    timestamp: 0,
                    value: Some("x".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // Last
                let got = iter.next().unwrap();
                assert_eq!(None, got);
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // Last
                let got = iter.next().unwrap();
                assert_eq!(None, got);
                // Last
                let got = iter.next().unwrap();
                assert_eq!(None, got);
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
            }

            #[test]
            fn two_steps_reverse_one_step_forward() {
                let mut iter = $alphabet(&(stringify!($name).to_string() + "::two_steps_reverse_one_step_forward"));
                iter.seek_to_last().unwrap();
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // Z
                let exp = KeyValuePair {
                    key: "Z".into(),
                    timestamp: 0,
                    value: Some("z".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // X
                let exp = KeyValuePair {
                    key: "X".into(),
                    timestamp: 0,
                    value: Some("x".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // Y
                let exp = KeyValuePair {
                    key: "Y".into(),
                    timestamp: 0,
                    value: Some("y".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // W-D
                for _ in 0..21 {
                    iter.prev().unwrap();
                    iter.prev().unwrap();
                    iter.next().unwrap();
                }
                // C
                let exp = KeyValuePair {
                    key: "C".into(),
                    timestamp: 0,
                    value: Some("c".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // C
                let exp = KeyValuePair {
                    key: "C".into(),
                    timestamp: 0,
                    value: Some("c".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // B
                let exp = KeyValuePair {
                    key: "B".into(),
                    timestamp: 0,
                    value: Some("b".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.prev().unwrap();
                assert_eq!(Some(exp), got);
                // First
                let got = iter.prev().unwrap();
                assert_eq!(None, got);
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
                // First
                let got = iter.prev().unwrap();
                assert_eq!(None, got);
                // First
                let got = iter.prev().unwrap();
                assert_eq!(None, got);
                // A
                let exp = KeyValuePair {
                    key: "A".into(),
                    timestamp: 0,
                    value: Some("a".into()),
                };
                let got = iter.next().unwrap();
                assert_eq!(Some(exp), got);
            }
        }
    )*
    }
}
