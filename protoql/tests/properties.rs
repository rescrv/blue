extern crate proptest;

use proptest::prelude::{any, ProptestConfig, Strategy};

use protoql::parser::{self, parse_all};
use protoql::{
    DataType, Direction, Field, FieldDefinition, FieldNumber, Identifier, Join, Key, KeyDataType,
    Map, Object, Table, TableSet,
};

proptest::prop_compose! {
    pub fn arb_identifier()(id in "[a-zA-Z][_a-zA-Z0-9]*") -> Identifier {
        Identifier::must(id)
    }
}

fn arb_field_number() -> impl Strategy<Value = FieldNumber> {
    proptest::prop_oneof! {
        (1u32..19000).prop_map(FieldNumber::must),
        (20000u32..(1<<29)).prop_map(FieldNumber::must),
    }
}

fn arb_data_type() -> impl Strategy<Value = DataType> {
    use proptest::prelude::Just;
    proptest::prop_oneof! {
        Just(DataType::unit),
        Just(DataType::int32),
        Just(DataType::int64),
        Just(DataType::uint32),
        Just(DataType::uint64),
        Just(DataType::sint32),
        Just(DataType::sint64),
        Just(DataType::fixed32),
        Just(DataType::fixed64),
        Just(DataType::sfixed32),
        Just(DataType::sfixed64),
        Just(DataType::float),
        Just(DataType::double),
        Just(DataType::Bool),
        Just(DataType::bytes),
        Just(DataType::bytes16),
        Just(DataType::bytes32),
        Just(DataType::bytes64),
        Just(DataType::string),
        Just(DataType::message),
    }
}

fn arb_key_data_type() -> impl Strategy<Value = KeyDataType> {
    use proptest::prelude::Just;
    proptest::prop_oneof! {
        Just(KeyDataType::fixed32),
        Just(KeyDataType::fixed64),
        Just(KeyDataType::sfixed32),
        Just(KeyDataType::sfixed64),
        Just(KeyDataType::string),
    }
}

proptest::prop_compose! {
    fn arb_field()(ident in arb_identifier(), number in arb_field_number(), ty in arb_data_type(), breakout in any::<bool>()) -> Field {
        Field::new(ident, number, ty, breakout).unwrap()
    }
}

proptest::prop_compose! {
    fn arb_key()(ident in arb_identifier(), number in arb_field_number(), ty in arb_key_data_type()) -> Key {
        Key::new(ident, number, ty, Direction::Forward).unwrap()
    }
}

proptest::prop_compose! {
    fn arb_object()(ident in arb_identifier(), number in arb_field_number()) -> Object {
        Object::new(ident, number, vec![]).unwrap()
    }
}

proptest::prop_compose! {
    fn arb_map()(key_ident in arb_identifier(), key_number in arb_field_number(), key_ty in arb_key_data_type()) -> Map {
        Map::new(Key::new(key_ident, key_number, key_ty, Direction::Forward).unwrap(), vec![]).unwrap()
    }
}

proptest::prop_compose! {
    fn arb_join()(ident in arb_identifier(), number in arb_field_number(), join_table in arb_identifier(), join_keys in proptest::collection::vec(arb_identifier(), 32)) -> Join {
        Join::new(ident, number, join_table, join_keys).unwrap()
    }
}

fn arb_level_1() -> impl Strategy<Value = FieldDefinition> {
    proptest::prop_oneof! {
        arb_field().prop_map(FieldDefinition::Field),
        arb_object().prop_map(FieldDefinition::Object),
        arb_map().prop_map(FieldDefinition::Map),
        arb_join().prop_map(FieldDefinition::Join),
    }
}

proptest::prop_compose! {
    fn arb_object1()(ident in arb_identifier(),
                     number in arb_field_number(),
                     fields in proptest::collection::vec(arb_level_1(), 0..32).prop_filter("duplicates", |f| protoql::check_fields(f).is_ok())) -> Object {
        Object::new(ident, number, fields).unwrap()
    }
}

proptest::prop_compose! {
    fn arb_map1()(key_ident in arb_identifier(),
                  key_number in arb_field_number(),
                  key_ty in arb_key_data_type(),
                  fields in proptest::collection::vec(arb_level_1(), 0..32).prop_filter("duplicates", |f| protoql::check_fields(f).is_ok())) -> Map {
        Map::new(Key::new(key_ident, key_number, key_ty, Direction::Forward).unwrap(), fields).unwrap()
    }
}

fn arb_level_2() -> impl Strategy<Value = FieldDefinition> {
    proptest::prop_oneof! {
        arb_field().prop_map(FieldDefinition::Field),
        arb_object1().prop_map(FieldDefinition::Object),
        arb_map1().prop_map(FieldDefinition::Map),
        arb_join().prop_map(FieldDefinition::Join),
    }
}

proptest::prop_compose! {
    fn arb_object2()(ident in arb_identifier(),
                     number in arb_field_number(),
                     fields in proptest::collection::vec(arb_level_2(), 0..32).prop_filter("duplicates", |f| protoql::check_fields(f).is_ok())) -> Object {
        Object::new(ident, number, fields).unwrap()
    }
}

proptest::prop_compose! {
    fn arb_map2()(key_ident in arb_identifier(),
                  key_number in arb_field_number(),
                  key_ty in arb_key_data_type(),
                  fields in proptest::collection::vec(arb_level_2(), 0..32).prop_filter("duplicates", |f| protoql::check_fields(f).is_ok())) -> Map {
        Map::new(Key::new(key_ident, key_number, key_ty, Direction::Forward).unwrap(), fields).unwrap()
    }
}

fn arb_field_definition() -> impl Strategy<Value = FieldDefinition> {
    proptest::prop_oneof! {
        arb_field().prop_map(FieldDefinition::Field),
        arb_object2().prop_map(FieldDefinition::Object),
        arb_map2().prop_map(FieldDefinition::Map),
        arb_join().prop_map(FieldDefinition::Join),
    }
}

proptest::prop_compose! {
    fn arb_table()(identifier in arb_identifier(),
                   number in arb_field_number(),
                   key in proptest::collection::vec(arb_key(), 0..5),
                   fields in proptest::collection::vec(arb_level_2(), 0..32).prop_filter("duplicates", |f| protoql::check_fields(f).is_ok())) -> Table {
        Table::new(identifier, number, key, fields).unwrap()
    }
}

proptest::prop_compose! {
    fn arb_table_set()(
                    tables in proptest::collection::vec(arb_table(), 0..8).prop_filter("duplicates", |t| protoql::check_tables(t).is_ok())) -> TableSet {
        TableSet::new(tables).unwrap()
    }
}

proptest::proptest! {
    #[test]
    fn ident(ident in arb_identifier()) {
        Identifier::parse(ident.to_protoql()).unwrap();
    }

    #[test]
    fn field_number_must(number in arb_field_number()) {
        assert!(FieldNumber::is_valid(number.get()));
    }

    #[test]
    fn data_type_must(data_type in arb_data_type()) {
        let s = data_type.to_protoql();
        assert_eq!(Ok(data_type), parse_all(parser::data_type)(s));
    }

    #[test]
    fn field_serializes_and_parses(field in arb_field()) {
        let exp = field.clone();
        let s = field.to_protoql();
        assert_eq!(Ok(exp), Field::parse(s));
    }

    #[test]
    fn object_serializes_and_parses(object in arb_object()) {
        let exp = object.clone();
        let s = object.to_protoql();
        assert_eq!(Ok(exp), Object::parse(s));
    }

    #[test]
    fn map_serializes_and_parses(map in arb_map()) {
        let exp = map.clone();
        let s = map.to_protoql();
        assert_eq!(Ok(exp), Map::parse(s));
    }
}

proptest::proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10, .. ProptestConfig::default()
    })]

    #[test]
    fn recursive_field_definition_serializes_and_parses(fd in arb_field_definition()) {
        let exp = fd.clone();
        let s = fd.to_protoql();
        assert_eq!(Ok(exp), FieldDefinition::parse(s));
    }

    #[test]
    fn table_serializes_and_parses(table in arb_table()) {
        let s = table.to_protoql();
        let exp = table;
        assert_eq!(Ok(exp), Table::parse(s));
    }
}

proptest::proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1, .. ProptestConfig::default()
    })]

    #[test]
    fn table_set_serializes_and_parses(table_set in arb_table_set()) {
        let s = table_set.to_protoql();
        let exp = table_set;
        assert_eq!(Ok(exp), TableSet::parse(s));
    }
}
