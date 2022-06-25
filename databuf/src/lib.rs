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
    uuid,
    message { what: Box<DataType> },
}

impl Default for DataType {
    fn default() -> Self {
        Self::unit
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DataType::unit => { write!(f, "unit") },
            DataType::int32 => { write!(f, "int32") },
            DataType::int64 => { write!(f, "int64") },
            DataType::uint32 => { write!(f, "uint32") },
            DataType::uint64 => { write!(f, "uint64") },
            DataType::sint32 => { write!(f, "sint32") },
            DataType::sint64 => { write!(f, "sint64") },
            DataType::Bool => { write!(f, "Bool") },
            DataType::fixed32 => { write!(f, "fixed32") },
            DataType::fixed64 => { write!(f, "fixed64") },
            DataType::sfixed32 => { write!(f, "sfixed32") },
            DataType::sfixed64 => { write!(f, "sfixed64") },
            DataType::float => { write!(f, "float") },
            DataType::double => { write!(f, "double") },
            DataType::bytes => { write!(f, "bytes") },
            DataType::string => { write!(f, "string") },
            DataType::uuid => { write!(f, "uuid") },
            DataType::message { what } => { write!(f, "{{{}}}", what) },
        }
    }
}

/////////////////////////////////////////////// Field //////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Field {
    Defined { number: u32, name: Identifier, data_type: DataType },
    Reserved { number: u32 },
}

impl std::fmt::Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Field::Defined { number, name, data_type } => {
                write!(f, "Field({} {} = {})", data_type, name.ident, number)
            },
            Field::Reserved { number } => {
                write!(f, "Field(reserved {})", number)
            }
        }
    }
}

///////////////////////////////////////////// FieldList ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FieldList {
    fields: Vec<Field>,
}

impl FieldList {
    pub fn from_vec(fields: Vec<Field>) -> Result<Self, String> {
        let mut fl = FieldList::default();
        for field in fields.iter() {
            fl.add_field(field.clone())?;
        }
        Ok(fl)
    }

    pub fn add_field(&mut self, field: Field) -> Result<(), String> {
        let (field_number, field_name) = match &field {
            Field::Defined { number, name, data_type: _ } => (number, Some(name)),
            Field::Reserved { number, } => (number, None),
        };
        for f in self.fields.iter() {
            match &f {
                Field::Defined { number, name, data_type: _ } => {
                    if number == field_number {
                        return Err(format!("{} collides with {} on number {}", field, f, number));
                    }
                    if Some(name) == field_name {
                        return Err(format!("{} collides with {} on name \"{}\"", field, f, name.ident));
                    }
                }
                Field::Reserved { number } => {
                    if number == field_number {
                        return Err(format!("{} collides with {} on number {}", field, f, number));
                    }
                }
            }
        }
        self.fields.push(field);
        Ok(())
    }
}

////////////////////////////////////////// TableDefinition /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableDefinition {
    name: Identifier,
    key: Vec<Identifier>,
    fields: FieldList,
}

impl TableDefinition {
    pub fn new(name: Identifier, key: Vec<Identifier>, fields: FieldList) -> Result<Self, String> {
        let table = Self {
            name,
            key,
            fields,
        };
        for k in table.key.iter() {
            if !table.has_field_name(k) {
                return Err(format!("table {} has key element {}, but no field name {} exists",
                                   table.name.ident, k.ident, k.ident));
            }
        }
        Ok(table)
    }

    pub fn has_field_name(&self, name: &Identifier) -> bool {
        for f in self.fields.fields.iter() {
            match f {
                Field::Defined { number: _, name: n, data_type: _ } => {
                    if n == name {
                        return true;
                    }
                },
                Field::Reserved { number: _, } => {},
            };
        }
        false
    }
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
    fn schema_data_type_uuid() {
        assert_eq!(DataType::uuid, schema::FieldTypeParser::new().parse("uuid").unwrap());
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

    #[test]
    fn schema_field_list() {
        let mut exp = FieldList::default();
        exp.add_field(Field::Defined {
            number: 5,
            name: Identifier::new("defined").unwrap(),
            data_type: DataType::int64,
        }).unwrap();
        exp.add_field(Field::Reserved {
            number: 7,
        }).unwrap();
        assert_eq!(exp, schema::FieldListParser::new().parse("int64 defined = 5; reserved 7;").unwrap());
    }

    #[test]
    fn schema_field_list_number_collision1() {
        let mut fl = FieldList::default();
        let exp = Ok(());
        assert_eq!(exp, fl.add_field(Field::Defined {
            number: 5,
            name: Identifier::new("defined").unwrap(),
            data_type: DataType::int64,
        }));
        let exp = Err("Field(int64 other = 5) collides with Field(int64 defined = 5) on number 5".to_string());
        assert_eq!(exp, fl.add_field(Field::Defined {
            number: 5,
            name: Identifier::new("other").unwrap(),
            data_type: DataType::int64,
        }));
    }

    #[test]
    fn schema_field_list_number_collision2() {
        let mut fl = FieldList::default();
        let exp = Ok(());
        assert_eq!(exp, fl.add_field(Field::Defined {
            number: 5,
            name: Identifier::new("defined").unwrap(),
            data_type: DataType::int64,
        }));
        let exp = Err("Field(int64 defined = 4) collides with Field(int64 defined = 5) on name \"defined\"".to_string());
        assert_eq!(exp, fl.add_field(Field::Defined {
            number: 4,
            name: Identifier::new("defined").unwrap(),
            data_type: DataType::int64,
        }));
    }

    #[test]
    fn schema_field_list_number_collision3() {
        let mut fl = FieldList::default();
        let exp = Ok(());
        assert_eq!(exp, fl.add_field(Field::Defined {
            number: 5,
            name: Identifier::new("defined").unwrap(),
            data_type: DataType::int64,
        }));
        let exp = Err("Field(reserved 5) collides with Field(int64 defined = 5) on number 5".to_string());
        assert_eq!(exp, fl.add_field(Field::Reserved {
            number: 5,
        }));
    }

    #[test]
    fn schema_identifier_list() {
        let mut identifiers = Vec::new();
        identifiers.push(Identifier::new("foo").unwrap());
        identifiers.push(Identifier::new("bar").unwrap());
        identifiers.push(Identifier::new("baz").unwrap());
        assert_eq!(identifiers, schema::IdentifierListParser::new().parse("foo, bar, baz").unwrap());
        assert_eq!(identifiers, schema::IdentifierListParser::new().parse("foo, bar, baz,").unwrap());
    }

    #[test]
    fn schema_table_definition() {
        let mut fields = FieldList::default();
        fields.add_field(Field::Defined {
            number: 1,
            name: Identifier::new("id").unwrap(),
            data_type: DataType::bytes,
        }).unwrap();
        fields.add_field(Field::Defined {
            number: 2,
            name: Identifier::new("email").unwrap(),
            data_type: DataType::string,
        }).unwrap();
        let users = Identifier::new("Users").unwrap();
        let id = Identifier::new("id").unwrap();
        let key = vec![id];
        let table = TableDefinition::new(users, key, fields);
        assert_eq!(table, schema::TableDefinitionParser::new().parse("table Users (id) {\
            bytes id = 1;\
            string email = 2;\
        }").unwrap());
    }
}
