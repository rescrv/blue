#![allow(clippy::excessive_precision)]
#![allow(clippy::approx_constant)]

use indicio::{Map, Value, value};

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

#[test]
fn map_lookup_returns_first_duplicate_key() {
    let mut map = Map::default();
    map.insert("key".to_string(), Value::I64(1));
    map.insert("key".to_string(), Value::I64(2));

    assert_eq!(2, map.len());
    assert!(!map.is_empty());
    assert_eq!(Some(&Value::I64(1)), map.get("key"));
    assert_eq!(
        vec![
            ("key".to_string(), Value::I64(1)),
            ("key".to_string(), Value::I64(2)),
        ],
        map.entries()
            .iter()
            .cloned()
            .map(indicio::MapEntry::into_pair)
            .collect::<Vec<_>>()
    );
}

#[test]
fn display_escapes_string_values_and_object_keys() {
    let mut map = Map::default();
    map.insert("a\n\"b".to_string(), Value::String("x\t\\\r".to_string()));

    assert_eq!(
        "{\"a\\n\\\"b\": \"x\\t\\\\\\r\"}",
        Value::Object(map).to_string()
    );
}

#[test]
fn value_accessors_report_kinds() {
    let value = value!({
        enabled: true,
        signed: -1i64,
        unsigned: 1u64,
        nested: ["value"],
    });

    assert_eq!(indicio::ValueKind::Object, value.kind());
    assert_eq!(Some(true), value.lookup("enabled").and_then(Value::as_bool));
    assert_eq!(Some(-1), value.lookup("signed").and_then(Value::as_i64));
    assert_eq!(Some(1), value.lookup("unsigned").and_then(Value::as_u64));
    assert_eq!(
        Some("value"),
        value
            .lookup_path(&["nested"])
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
    );
}
