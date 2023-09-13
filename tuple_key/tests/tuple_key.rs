use std::fmt::Debug;

use tuple_key::{TupleKey, TypedTupleKey};
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
    test_helper(EmptyTupleKey{}, &[])
}

///////////////////////////////////////// AllTypesTupleKey /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, TypedTupleKey)]
struct AllTypesTupleKey {
    #[tuple_key(1, message)]
    fixed_32: u32,
    #[tuple_key(1, message)]
    fixed_64: u64,
    #[tuple_key(1, message)]
    sfixed_32: i32,
    #[tuple_key(1, message)]
    sfixed_64: i64,
    #[tuple_key(1, message)]
    bytes: Vec<u8>,
    #[tuple_key(1, message)]
    bytes_16: [u8; 16],
    #[tuple_key(1, message)]
    bytes_32: [u8; 32],
    #[tuple_key(1, message)]
    string: String,
}

#[test]
fn all_types_tuple_key() {
    test_helper(AllTypesTupleKey {
        fixed_32: 0x1eaff00du32,
        fixed_64: 0xc0ffee00c0ffee00u64,
        sfixed_32: 0x1eaff00di32,
        sfixed_64: 0xc0ffee00c0ffee00u64 as i64,
        bytes: vec![0, 1, 2, 3],
        bytes_16: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        bytes_32: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31],
        string: "hello world".to_owned(),
	}, &[62, 2, 31, 87, 253, 1, 208, 62, 4, 193, 127, 251, 193, 13, 7, 255, 221, 1, 0, 62, 6, 159, 87, 253, 1, 208, 62, 8, 65, 127, 251, 193, 13, 7, 255, 221, 1, 0, 62, 10, 1, 1, 65, 65, 48, 62, 12, 1, 1, 65, 65, 49, 33, 21, 13, 7, 133, 3, 33, 161, 89, 49, 27, 15, 7, 192, 62, 14, 1, 1, 65, 65, 49, 33, 21, 13, 7, 133, 3, 33, 161, 89, 49, 27, 15, 7, 197, 3, 17, 145, 77, 41, 21, 139, 133, 227, 129, 201, 105, 55, 29, 15, 71, 195, 240, 62, 16, 105, 51, 91, 141, 199, 121, 129, 239, 111, 185, 155, 141, 64]);
}

/////////////////////////////////////////// WithUnitTypes //////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, TypedTupleKey)]
struct WithUnitTypes {
    #[tuple_key(1, message)]
    unit1: (),
    #[tuple_key(2, message)]
    unit2: (),
}

#[test]
fn with_unit_types() {
    test_helper(WithUnitTypes {
        unit1: (),
        unit2: (),
    }, &[62, 94]);
}
