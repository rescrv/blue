use std::fmt::Debug;

use prototk::FieldNumber;

use tuple_key::{Direction, KeyDataType, TupleKey, TupleKeyParser, TypedTupleKey};
use tuple_key_derive::TypedTupleKey;

//////////////////////////////////////////// test helper ///////////////////////////////////////////

fn test_helper<T: TypedTupleKey + Clone + Debug + Eq>(typed: T, bytes: &[u8])
where
    <T as TryFrom<TupleKey>>::Error: Debug,
{
    let tk = <T as Into<TupleKey>>::into(typed.clone());
    assert_eq!(bytes, tk.as_bytes());
    let got = <T as TryFrom<TupleKey>>::try_from(tk).unwrap();
    assert_eq!(typed, got);
}

/////////////////////////////////////////// EmptyTupleKey //////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, TypedTupleKey)]
struct EmptyTupleKey {}

#[test]
fn empty_tuple_key() {
    test_helper(EmptyTupleKey {}, &[])
}

///////////////////////////////////////// AllTypesTupleKey /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, TypedTupleKey)]
struct AllTypesTupleKey {
    #[tuple_key(1)]
    unit: (),
    #[tuple_key(1)]
    fixed_32: u32,
    #[tuple_key(1)]
    fixed_64: u64,
    #[tuple_key(1)]
    sfixed_32: i32,
    #[tuple_key(1)]
    sfixed_64: i64,
    #[tuple_key(1)]
    string: String,
}

#[test]
fn all_types_tuple_key() {
    test_helper(
        AllTypesTupleKey {
            unit: (),
            fixed_32: 0x1eaff00du32,
            fixed_64: 0xc0ffee00c0ffee00u64,
            sfixed_32: 0x1eaff00di32,
            sfixed_64: 0xc0ffee00c0ffee00u64 as i64,
            string: "hello world".to_owned(),
        },
        &[
            34, 0, // unit
            36, 31, 87, 253, 1, 208, // fixed32
            38, 193, 127, 251, 193, 13, 7, 255, 221, 1, 0, // fixed32
            40, 159, 87, 253, 1, 208, // sfixed32
            42, 65, 127, 251, 193, 13, 7, 255, 221, 1, 0, // sfixed64
            44, 105, 51, 91, 141, 199, 121, 129, 239, 111, 185, 155, 141, 64, // string
        ],
    );
}

///////////////////////////////////////// AllTypesTupleKey /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, TypedTupleKey)]
struct AllTypesTupleKeyReverse {
    #[tuple_key(1)]
    unit: (),
    #[tuple_key(1)]
    #[reverse]
    fixed_32: u32,
    #[tuple_key(1)]
    #[reverse]
    fixed_64: u64,
    #[tuple_key(1)]
    #[reverse]
    sfixed_32: i32,
    #[tuple_key(1)]
    #[reverse]
    sfixed_64: i64,
    #[tuple_key(1)]
    #[reverse]
    string: String,
}

#[test]
fn all_types_tuple_key_reverse() {
    test_helper(
        AllTypesTupleKeyReverse {
            unit: (),
            fixed_32: 0x1eaff00du32,
            fixed_64: 0xc0ffee00c0ffee00u64,
            sfixed_32: 0x1eaff00di32,
            sfixed_64: 0xc0ffee00c0ffee00u64 as i64,
            string: "hello world".to_owned(),
        },
        &[
            34, 0, // unit
            52, 225, 169, 3, 255, 46, // fixed32
            54, 63, 129, 5, 63, 243, 249, 1, 35, 255, 254, // fixed64
            56, 97, 169, 3, 255, 46, // sfixed32
            58, 191, 129, 5, 63, 243, 249, 1, 35, 255, 254, // sfixed64
            60, 151, 205, 165, 115, 57, 135, 127, 17, 145, 71, 101, 115, 190, // string
        ],
    );
}

/////////////////////////////////////////// WithUnitTypes //////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, TypedTupleKey)]
struct WithUnitTypes {
    #[tuple_key(1)]
    unit1: (),
    #[tuple_key(2)]
    unit2: (),
}

#[test]
fn with_unit_types() {
    test_helper(
        WithUnitTypes {
            unit1: (),
            unit2: (),
        },
        &[34, 0, 66, 0],
    );
}

///////////////////////////////////////////// proptest /////////////////////////////////////////////

use proptest::prelude::{any, Strategy};

fn arb_key_data_type() -> impl Strategy<Value = KeyDataType> {
    use proptest::prelude::Just;
    proptest::prop_oneof! {
        Just(KeyDataType::unit),
        Just(KeyDataType::fixed32),
        Just(KeyDataType::fixed64),
        Just(KeyDataType::sfixed32),
        Just(KeyDataType::sfixed64),
        Just(KeyDataType::string),
    }
}

fn arb_direction() -> impl Strategy<Value = Direction> {
    use proptest::prelude::Just;
    proptest::prop_oneof! {
        Just(Direction::Forward),
        Just(Direction::Reverse),
    }
}

fn arb_field_number() -> impl Strategy<Value = FieldNumber> {
    proptest::prop_oneof! {
        (1u32..19000).prop_map(FieldNumber::must),
        (20000u32..(1<<29)).prop_map(FieldNumber::must),
    }
}

#[derive(Debug, Eq, PartialEq)]
enum TestElement {
    Unit,
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    String(String),
}

impl TestElement {
    fn key_data_type(&self) -> KeyDataType {
        match self {
            TestElement::Unit => KeyDataType::unit,
            TestElement::I32(_) => KeyDataType::sfixed32,
            TestElement::U32(_) => KeyDataType::fixed32,
            TestElement::I64(_) => KeyDataType::sfixed64,
            TestElement::U64(_) => KeyDataType::fixed64,
            TestElement::String(_) => KeyDataType::string,
        }
    }

    fn extend(&self, f: FieldNumber, d: Direction, key: &mut TupleKey) {
        match self {
            TestElement::Unit => key.extend(f),
            TestElement::I32(e) => key.extend_with_key(f, *e, d),
            TestElement::U32(e) => key.extend_with_key(f, *e, d),
            TestElement::I64(e) => key.extend_with_key(f, *e, d),
            TestElement::U64(e) => key.extend_with_key(f, *e, d),
            TestElement::String(e) => key.extend_with_key(f, e.clone(), d),
        }
    }

    fn parse(&self, f: FieldNumber, d: Direction, key: &mut TupleKeyParser) {
        match self {
            TestElement::Unit => {
                assert_eq!(Direction::Forward, d);
                key.parse_next(f, d).unwrap()
            }
            TestElement::I32(e) => assert_eq!(*e, key.parse_next_with_key(f, d).unwrap()),
            TestElement::U32(e) => assert_eq!(*e, key.parse_next_with_key(f, d).unwrap()),
            TestElement::I64(e) => assert_eq!(*e, key.parse_next_with_key(f, d).unwrap()),
            TestElement::U64(e) => assert_eq!(*e, key.parse_next_with_key(f, d).unwrap()),
            TestElement::String(e) => {
                assert_eq!(e, &key.parse_next_with_key::<String>(f, d).unwrap())
            }
        }
    }
}

fn arb_element() -> impl Strategy<Value = TestElement> {
    proptest::prop_oneof! {
        (0..1).prop_map(|_| TestElement::Unit),
        any::<i32>().prop_map(TestElement::I32),
        any::<u32>().prop_map(TestElement::U32),
        any::<i64>().prop_map(TestElement::I64),
        any::<u64>().prop_map(TestElement::U64),
        ".*".prop_map(|e| TestElement::String(e.to_string())),
    }
}

proptest::proptest! {
    #[test]
    fn discriminant(ty in arb_key_data_type(), dir in arb_direction()) {
        let d = tuple_key::to_discriminant(ty, dir);
        assert_eq!(Some((ty, dir)), tuple_key::from_discriminant(d))
    }

    #[test]
    fn tuple_key_proptest(elements in proptest::collection::vec((arb_field_number(), arb_direction(), arb_element()), 0..32)
                              .prop_filter("reverse unit", |v| v.iter().all(|e| (e.1 != Direction::Reverse || e.2 != TestElement::Unit)))) {
        let mut key = TupleKey::default();
        for (field_number, direction, element) in elements.iter() {
            element.extend(*field_number, *direction, &mut key);
        }
        let mut parser = TupleKeyParser::new(&key);
        for (field_number, direction, element) in elements.iter() {
            let (f, k, d) = parser.peek_next().unwrap().unwrap();
            assert_eq!(*field_number, f);
            assert_eq!(element.key_data_type(), k);
            assert_eq!(*direction, d);
            element.parse(*field_number, *direction, &mut parser);
        }
    }
}
