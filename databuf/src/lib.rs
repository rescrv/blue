#[macro_use]
extern crate lalrpop_util;

lalrpop_mod!(pub schema);

//////////////////////////////////////////// Identifier ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Identifier {
    ident: String,
}

impl Identifier {
    fn new(what: &str) -> Option<Self> {
        const ALPHA: &str = "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        const DIGITS: &str = "0123456789";
        if what.len() < 1 {
            return None;
        }
        let mut chars = what.chars();
        let first = chars.next();
        if first == None {
            return None;
        }
        if !ALPHA.contains(first.unwrap()) {
            return None;
        }
        for c in chars {
            if !ALPHA.contains(c) && !DIGITS.contains(c) {
                return None;
            }
        }
        let ident = Identifier {
            ident: what.to_string(),
        };
        Some(ident)
    }
}

///////////////////////////////////////////// DataType /////////////////////////////////////////////

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataType {
    unit,
    int32,
    int64,
    uint32,
    uint64,
    sint32,
    sint64,
    Bool,
    fixed32,
    fixed64,
    sfixed32,
    sfixed64,
    float,
    double,
    bytes,
    string,
    message { what: Box<DataType> },
}

impl Default for DataType {
    fn default() -> Self {
        Self::unit
    }
}

/////////////////////////////////////////////// Field //////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Field {
    Defined { number: u32, name: Identifier, data_type: DataType },
    Reserved { number: u32 },
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_field_number() {
        assert_eq!(1, schema::FieldNumberParser::new().parse("1").unwrap());
        assert_eq!(512, schema::FieldNumberParser::new().parse("512").unwrap());
    }

    #[test]
    fn schema_data_type_int32() {
        assert_eq!(DataType::int32, schema::FieldTypeParser::new().parse("int32").unwrap());
    }

    #[test]
    fn schema_data_type_int64() {
        assert_eq!(DataType::int64, schema::FieldTypeParser::new().parse("int64").unwrap());
    }

    #[test]
    fn schema_data_type_uint32() {
        assert_eq!(DataType::uint32, schema::FieldTypeParser::new().parse("uint32").unwrap());
    }

    #[test]
    fn schema_data_type_uint64() {
        assert_eq!(DataType::uint64, schema::FieldTypeParser::new().parse("uint64").unwrap());
    }

    #[test]
    fn schema_data_type_sint32() {
        assert_eq!(DataType::sint32, schema::FieldTypeParser::new().parse("sint32").unwrap());
    }

    #[test]
    fn schema_data_type_sint64() {
        assert_eq!(DataType::sint64, schema::FieldTypeParser::new().parse("sint64").unwrap());
    }

    #[test]
    fn schema_data_type_bool() {
        assert_eq!(DataType::Bool, schema::FieldTypeParser::new().parse("Bool").unwrap());
    }

    #[test]
    fn schema_data_type_fixed32() {
        assert_eq!(DataType::fixed32, schema::FieldTypeParser::new().parse("fixed32").unwrap());
    }

    #[test]
    fn schema_data_type_fixed64() {
        assert_eq!(DataType::fixed64, schema::FieldTypeParser::new().parse("fixed64").unwrap());
    }

    #[test]
    fn schema_data_type_sfixed32() {
        assert_eq!(DataType::sfixed32, schema::FieldTypeParser::new().parse("sfixed32").unwrap());
    }

    #[test]
    fn schema_data_type_sfixed64() {
        assert_eq!(DataType::sfixed64, schema::FieldTypeParser::new().parse("sfixed64").unwrap());
    }

    #[test]
    fn schema_data_type_float() {
        assert_eq!(DataType::float, schema::FieldTypeParser::new().parse("float").unwrap());
    }

    #[test]
    fn schema_data_type_double() {
        assert_eq!(DataType::double, schema::FieldTypeParser::new().parse("double").unwrap());
    }

    #[test]
    fn schema_data_type_bytes() {
        assert_eq!(DataType::bytes, schema::FieldTypeParser::new().parse("bytes").unwrap());
    }

    #[test]
    fn schema_data_type_string() {
        assert_eq!(DataType::string, schema::FieldTypeParser::new().parse("string").unwrap());
    }

    #[test]
    fn schema_field_defined() {
        let exp = Field::Defined {
            number: 5,
            name: Identifier::new("defined").unwrap(),
            data_type: DataType::int64,
        };
        assert_eq!(exp, schema::FieldParser::new().parse("int64 defined = 5").unwrap());
    }

    #[test]
    fn schema_field_reserved() {
        let exp = Field::Reserved {
            number: 5,
        };
        assert_eq!(exp, schema::FieldParser::new().parse("reserved 5").unwrap());
    }
}
