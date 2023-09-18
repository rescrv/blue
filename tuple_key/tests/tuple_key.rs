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
    bytes: Vec<u8>,
    #[tuple_key(1)]
    bytes_16: [u8; 16],
    #[tuple_key(1)]
    bytes_32: [u8; 32],
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
            bytes: vec![0, 1, 2, 3],
            bytes_16: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            bytes_32: [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31,
            ],
            string: "hello world".to_owned(),
        },
        &[
            32, 0, // unit
            34, 31, 87, 253, 1, 208, // fixed32
            36, 193, 127, 251, 193, 13, 7, 255, 221, 1, 0, // fixed32
            38, 159, 87, 253, 1, 208, // sfixed32
            40, 65, 127, 251, 193, 13, 7, 255, 221, 1, 0, // sfixed64
            42, 1, 1, 65, 65, 48, // bytes
            44, 1, 1, 65, 65, 49, 33, 21, 13, 7, 133, 3, 33, 161, 89, 49, 27, 15, 7, 192, // bytes16
            46, 1, 1, 65, 65, 49, 33, 21, 13, 7, 133, 3, 33, 161, 89, 49, 27, 15, 7, 197, // bytes32
                3, 17, 145, 77, 41, 21, 139, 133, 227, 129, 201, 105, 55, 29, 15, 71, 195, 240, // bytes32 cont'd
            48, 105, 51, 91, 141, 199, 121, 129, 239, 111, 185, 155, 141, 64, // string
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
        &[32, 0, 64, 0],
    );
}
