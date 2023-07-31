use std::fmt::Debug;

use tuple_key::TupleKey;
use tuple_key::FromIntoTupleKey;
use tuple_key_derive::FromIntoTupleKey;

///////////////////////////////////// test_from_into_tuple_key /////////////////////////////////////

fn test_from_into_tuple_key<TK: FromIntoTupleKey + Clone + Debug + Eq>(ttk: TK, bytes: &[u8]) {
    let tk = ttk.clone().into_tuple_key();
    assert_eq!(bytes, tk.as_bytes());
    let got = TK::from_tuple_key(&tk).unwrap();
    assert_eq!(ttk, got);
}

/////////////////////////////////////////// EmptyTupleKey //////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, FromIntoTupleKey)]
struct EmptyTupleKey();

#[test]
fn empty_tuple_key() {
    test_from_into_tuple_key::<EmptyTupleKey>(EmptyTupleKey::default(), &[]);
}

/////////////////////////////////////////// OneElementKey //////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, FromIntoTupleKey)]
struct OneElementKey (
    #[tuple_key(1)]
    u64,
);

#[test]
fn one_element_key() {
    test_from_into_tuple_key::<OneElementKey>(OneElementKey(42u64), &[((1 << 4) | 2) << 1, 4, 1, 1, 1, 1, 1, 1, 1, 1, 43, 0]);
}

///////////////////////////////////////////// StringKey ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, FromIntoTupleKey)]
struct StringKey (
    #[tuple_key(7)]
    String,
);

#[test]
fn string_key() {
    test_from_into_tuple_key::<StringKey>(StringKey("Hello World".to_owned()), &[((7 << 4) | 8) << 1, 16, 0x49, 0x33, 0x5b, 0x8d, 0xc7, 0x79, 0x81, 0xaf, 0x6f, 0xb9, 0x9b, 0x8d, 0x40]);
}

/////////////////////////////////////////// EmptyTriplet ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, FromIntoTupleKey)]
struct EmptyTriplet (
    #[tuple_key(7)]
    (),
    #[tuple_key(6)]
    (),
    #[tuple_key(5)]
    (),
);

#[test]
fn empty_triplet() {
    test_from_into_tuple_key::<EmptyTriplet>(EmptyTriplet((), (), ()), &[((7 << 4) | 15) << 1, ((6 << 4) | 15) << 1, ((5 << 4) | 15) << 1]); 
}

//////////////////////////////////////////// NamedFields ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, FromIntoTupleKey)]
struct StringDoublet {
    #[tuple_key(8)]
    first: String,
    #[tuple_key(9)]
    second: String,
}

#[test]
fn string_doublet() {
    let doublet = StringDoublet { first: "first".to_owned(), second: "second".to_owned() };
    let expected = &[17, 2, 16, 103, 53, 93, 79, 55, 160, 49, 2, 16, 115, 179, 89, 109, 247, 115, 144];
    test_from_into_tuple_key::<StringDoublet>(doublet, expected);
}
