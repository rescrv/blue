#![allow(clippy::excessive_precision)]
#![allow(clippy::approx_constant)]

use indicio::{value, Value};

#[test]
fn value_bool() {
    assert_eq!(Value::Bool(true), value!(true));
    assert_eq!(Value::Bool(false), value!(false));
}

#[test]
fn value_i64() {
    assert_eq!(Value::I64(i64::MIN), value!(-9223372036854775808i64));
    assert_eq!(Value::I64(i64::MAX), value!(9223372036854775807i64));
}

#[test]
fn value_u64() {
    assert_eq!(Value::U64(u64::MAX), value!(18446744073709551615u64));
}

#[test]
fn value_f64() {
    assert_eq!(
        Value::F64(std::f64::consts::PI),
        value!(3.14159265358979323846264338327950288_f64)
    );
}

#[test]
fn value_string() {
    assert_eq!(Value::String("foo".to_string()), value!("foo"));
}

#[test]
fn value_array() {
    assert_eq!(Value::Array(vec![].into()), value!([]));
    assert_eq!(
        Value::Array(
            vec![
                Value::Bool(false),
                Value::Bool(true),
                Value::Array(
                    vec![
                        Value::String("hello".to_string()),
                        Value::String("world".to_string())
                    ]
                    .into()
                )
            ]
            .into()
        ),
        value!([false, true, ["hello", "world"]])
    );
}

#[test]
fn value_object() {
    assert_eq!(Value::Object(vec![].into_iter().collect()), value!({}));
    assert_eq!(
        Value::Object(
            vec![
                ("hello".to_string(), Value::String("world".to_string())),
                (
                    "consts".to_string(),
                    Value::Array(
                        vec![Value::F64(2.718281828459045), Value::F64(3.141592653589793)].into()
                    )
                ),
                (
                    "recursive".to_string(),
                    Value::Object(
                        vec![
                            ("hello".to_string(), Value::String("world".to_string())),
                            (
                                "consts".to_string(),
                                Value::Array(
                                    vec![
                                        Value::F64(2.718281828459045),
                                        Value::F64(3.141592653589793)
                                    ]
                                    .into()
                                )
                            ),
                        ]
                        .into_iter()
                        .collect()
                    )
                ),
            ]
            .into_iter()
            .collect()
        ),
        value!({
            hello: "world",
            consts: [
                2.71828182845904523536028747135266250_f64,
                3.14159265358979323846264338327950288_f64,
            ],
            recursive: {
                hello: "world",
                consts: [
                    2.71828182845904523536028747135266250_f64,
                    3.14159265358979323846264338327950288_f64,
                ],
            }
        })
    );
}
