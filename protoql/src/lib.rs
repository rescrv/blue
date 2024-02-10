use std::hash::Hash;

use buffertk::{v64, Unpackable};
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

pub use prototk::{FieldNumber, WireType};
pub use tuple_key::{Direction, KeyDataType, TupleKey, TupleKeyIterator};

pub mod object_builder;
pub mod parser;

pub use parser::ParseError;

// TODO(rescrv):  Support reverse keys.

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, prototk_derive::Message, zerror_derive::Z)]
pub enum Error {
    #[prototk(507904, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(507905, message)]
    TupleKeyError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        error: tuple_key::Error,
    },
    #[prototk(507906, message)]
    LogicError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(507907, message)]
    Corruption {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(507908, message)]
    DuplicateIdentifier {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        ident: String,
    },
    #[prototk(507909, message)]
    DuplicateFieldNumber {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, uint32)]
        number: u32,
    },
    #[prototk(507910, message)]
    InvalidKeyType {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        data_type: DataType,
    },
    #[prototk(507911, message)]
    BreakoutKey {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        ident: String,
    },
    #[prototk(507912, message)]
    ParseError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        err: String,
    },
    #[prototk(507913, message)]
    InvalidNumberLiteral {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        as_str: String,
    },
    #[prototk(507914, message)]
    SchemaIncompatibility {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(507915, message)]
    UnknownTable {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        ident: String,
    },
    #[prototk(507916, message)]
    UnknownField {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        ident: String,
    },
    #[prototk(507917, message)]
    InvalidSchema {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(507918, message)]
    InvalidQuery {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(507919, message)]
    InvalidKey {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(507920, message)]
    ExecutionError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
}

impl Default for Error {
    fn default() -> Self {
        Self::Success {
            core: ErrorCore::default(),
        }
    }
}

impl From<ParseError> for Error {
    fn from(err: ParseError) -> Self {
        Self::ParseError {
            core: ErrorCore::default(),
            err: err.to_string(),
        }
    }
}

iotoz! {Error}

///////////////////////////////////////////// DataType /////////////////////////////////////////////

#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, prototk_derive::Message,
)]
#[allow(non_camel_case_types)]
pub enum DataType {
    #[default]
    #[prototk(1, message)]
    unit,
    #[prototk(2, message)]
    int32,
    #[prototk(3, message)]
    int64,
    #[prototk(4, message)]
    uint32,
    #[prototk(5, message)]
    uint64,
    #[prototk(6, message)]
    sint32,
    #[prototk(7, message)]
    sint64,
    #[prototk(8, message)]
    fixed32,
    #[prototk(9, message)]
    fixed64,
    #[prototk(10, message)]
    sfixed32,
    #[prototk(11, message)]
    sfixed64,
    #[prototk(12, message)]
    timestamp_micros,
    #[prototk(13, message)]
    float,
    #[prototk(14, message)]
    double,
    #[prototk(15, message)]
    Bool,
    #[prototk(16, message)]
    bytes,
    #[prototk(17, message)]
    bytes16,
    #[prototk(18, message)]
    bytes32,
    #[prototk(19, message)]
    bytes64,
    #[prototk(20, message)]
    string,
    #[prototk(21, message)]
    message,
}

impl DataType {
    pub fn wire_type(self) -> WireType {
        match self {
            DataType::unit => WireType::LengthDelimited,
            DataType::int32 => WireType::Varint,
            DataType::int64 => WireType::Varint,
            DataType::uint32 => WireType::Varint,
            DataType::uint64 => WireType::Varint,
            DataType::sint32 => WireType::Varint,
            DataType::sint64 => WireType::Varint,
            DataType::fixed32 => WireType::ThirtyTwo,
            DataType::fixed64 => WireType::SixtyFour,
            DataType::sfixed32 => WireType::ThirtyTwo,
            DataType::sfixed64 => WireType::SixtyFour,
            DataType::timestamp_micros => WireType::SixtyFour,
            DataType::float => WireType::ThirtyTwo,
            DataType::double => WireType::SixtyFour,
            DataType::Bool => WireType::Varint,
            DataType::bytes => WireType::LengthDelimited,
            DataType::bytes16 => WireType::LengthDelimited,
            DataType::bytes32 => WireType::LengthDelimited,
            DataType::bytes64 => WireType::LengthDelimited,
            DataType::string => WireType::LengthDelimited,
            DataType::message => WireType::LengthDelimited,
        }
    }

    pub fn to_protoql(&self) -> &'static str {
        match self {
            DataType::unit => "unit",
            DataType::int32 => "int32",
            DataType::int64 => "int64",
            DataType::uint32 => "uint32",
            DataType::uint64 => "uint64",
            DataType::sint32 => "sint32",
            DataType::sint64 => "sint64",
            DataType::fixed32 => "fixed32",
            DataType::fixed64 => "fixed64",
            DataType::sfixed32 => "sfixed32",
            DataType::sfixed64 => "sfixed64",
            DataType::timestamp_micros => "timestamp_micros",
            DataType::float => "float",
            DataType::double => "double",
            DataType::Bool => "bool",
            DataType::bytes => "bytes",
            DataType::bytes16 => "bytes16",
            DataType::bytes32 => "bytes32",
            DataType::bytes64 => "bytes64",
            DataType::string => "string",
            DataType::message => "message",
        }
    }

    pub fn to_protobuf(&self) -> &'static str {
        match self {
            DataType::unit => "unit",
            DataType::int32 => "int32",
            DataType::int64 => "int64",
            DataType::uint32 => "uint32",
            DataType::uint64 => "uint64",
            DataType::sint32 => "sint32",
            DataType::sint64 => "sint64",
            DataType::fixed32 => "fixed32",
            DataType::fixed64 => "fixed64",
            DataType::sfixed32 => "sfixed32",
            DataType::sfixed64 => "sfixed64",
            DataType::timestamp_micros => "sfixed64",
            DataType::float => "float",
            DataType::double => "double",
            DataType::Bool => "bool",
            DataType::bytes => "bytes",
            DataType::bytes16 => "bytes",
            DataType::bytes32 => "bytes",
            DataType::bytes64 => "bytes",
            DataType::string => "string",
            DataType::message => "message",
        }
    }

    pub fn can_cast(lhs: Self, rhs: Self) -> bool {
        if lhs == rhs {
            return true;
        }
        matches! {
            (lhs, rhs),
            (DataType::unit, DataType::unit) |
            (DataType::int32, DataType::int32) |
            (DataType::int32, DataType::sfixed32) |
            (DataType::int32, DataType::sint32) |
            (DataType::sfixed32, DataType::int32) |
            (DataType::sfixed32, DataType::sfixed32) |
            (DataType::sfixed32, DataType::sint32) |
            (DataType::sint32, DataType::int32) |
            (DataType::sint32, DataType::sfixed32) |
            (DataType::sint32, DataType::sint32) |
            (DataType::int64, DataType::int64) |
            (DataType::int64, DataType::sfixed64) |
            (DataType::int64, DataType::sint64) |
            (DataType::sfixed64, DataType::int64) |
            (DataType::sfixed64, DataType::sfixed64) |
            (DataType::sfixed64, DataType::sint64) |
            (DataType::sint64, DataType::int64) |
            (DataType::sint64, DataType::sfixed64) |
            (DataType::sint64, DataType::sint64) |
            (DataType::uint32, DataType::fixed32) |
            (DataType::fixed32, DataType::uint32) |
            (DataType::uint64, DataType::fixed64) |
            (DataType::fixed64, DataType::uint64)
        }
    }

    pub fn to_key(&self) -> Option<KeyDataType> {
        match self {
            DataType::unit => None,
            DataType::int32 => None,
            DataType::int64 => None,
            DataType::uint32 => None,
            DataType::uint64 => None,
            DataType::sint32 => None,
            DataType::sint64 => None,
            DataType::fixed32 => Some(KeyDataType::fixed32),
            DataType::fixed64 => Some(KeyDataType::fixed64),
            DataType::sfixed32 => Some(KeyDataType::sfixed32),
            DataType::sfixed64 => Some(KeyDataType::sfixed64),
            DataType::timestamp_micros => Some(KeyDataType::sfixed64),
            DataType::float => None,
            DataType::double => None,
            DataType::Bool => None,
            DataType::bytes => None,
            DataType::bytes16 => None,
            DataType::bytes32 => None,
            DataType::bytes64 => None,
            DataType::string => Some(KeyDataType::string),
            DataType::message => None,
        }
    }
}

impl From<KeyDataType> for DataType {
    fn from(ty: KeyDataType) -> Self {
        match ty {
            KeyDataType::unit => DataType::unit,
            KeyDataType::fixed32 => DataType::fixed32,
            KeyDataType::fixed64 => DataType::fixed64,
            KeyDataType::sfixed32 => DataType::sfixed32,
            KeyDataType::sfixed64 => DataType::sfixed64,
            KeyDataType::string => DataType::string,
        }
    }
}

//////////////////////////////////////////// KeyLiteral ////////////////////////////////////////////

#[derive(Debug, Eq, PartialEq)]
#[allow(non_camel_case_types)]
pub enum KeyLiteral {
    fixed32 { value: u32 },
    fixed64 { value: u64 },
    sfixed32 { value: i32 },
    sfixed64 { value: i64 },
    timestamp_micros { value: i64 },
    string { value: String },
}

//////////////////////////////////////////// Identifier ////////////////////////////////////////////

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Identifier {
    ident: String,
}

impl Identifier {
    pub fn must<S: AsRef<str>>(ident: S) -> Self {
        Identifier::parse(ident).expect("parse to always succeed")
    }

    pub fn parse<S: AsRef<str>>(ident: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::identifier)(ident.as_ref())?)
    }

    pub fn to_camel_case(&self) -> String {
        let mut cc = String::new();
        for segment in self.ident.split('_') {
            if segment.is_empty() {
                continue;
            }
            let mut chars = segment.chars();
            if let Some(c) = chars.next() {
                cc += &c.to_uppercase().collect::<String>();
                cc += &chars.collect::<String>();
            }
        }
        cc
    }

    pub fn to_protoql(&self) -> &str {
        self.ident.as_str()
    }
}

impl std::fmt::Debug for Identifier {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Identifier({})", self.ident)
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}", self.ident)
    }
}

//////////////////////////////////////////////// Key ///////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Key {
    ident: Identifier,
    number: FieldNumber,
    ty: KeyDataType,
    dir: Direction,
}

impl Key {
    pub fn new(
        ident: Identifier,
        number: FieldNumber,
        ty: KeyDataType,
        dir: Direction,
    ) -> Result<Key, Error> {
        Ok(Self {
            ident,
            number,
            ty,
            dir,
        })
    }

    pub fn parse<S: AsRef<str>>(key: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::key)(key.as_ref())?)
    }

    pub fn to_protoql(&self) -> String {
        let ty: DataType = self.ty.into();
        format!("{} {} = {}", ty.to_protoql(), self.ident, self.number)
    }

    fn to_protobuf(&self) -> String {
        let ty: DataType = self.ty.into();
        format!(
            "optional {} {} = {}",
            ty.to_protobuf(),
            self.ident,
            self.number
        )
    }
}

/////////////////////////////////////////////// Field //////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Field {
    ident: Identifier,
    number: FieldNumber,
    ty: DataType,
    breakout: bool,
}

impl Field {
    pub fn new(
        ident: Identifier,
        number: FieldNumber,
        ty: DataType,
        breakout: bool,
    ) -> Result<Field, Error> {
        Ok(Self {
            ident,
            number,
            ty,
            breakout,
        })
    }

    pub fn breakout(&self) -> bool {
        self.breakout
    }

    pub fn parse<S: AsRef<str>>(field: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::field)(field.as_ref())?)
    }

    pub fn to_protoql(&self) -> String {
        format!(
            "{}{} {} = {}",
            if self.breakout { "breakout " } else { "" },
            self.ty.to_protoql(),
            self.ident,
            self.number
        )
    }

    fn to_protobuf(&self) -> String {
        format!(
            "optional {} {} = {}",
            self.ty.to_protobuf(),
            self.ident,
            self.number
        )
    }

    pub fn describe_keys(&self, prefix: &str) -> Vec<String> {
        let mut keys = vec![];
        let mut prefix = prefix.to_string();
        prefix += &format!(", {}:{}:{}", self.number, self.ident, self.ty.to_protobuf());
        if self.breakout {
            keys.push(prefix + ")");
        }
        keys
    }
}

impl From<&Key> for Field {
    fn from(key: &Key) -> Self {
        Self {
            ident: key.ident.clone(),
            number: key.number,
            ty: key.ty.into(),
            breakout: false,
        }
    }
}

////////////////////////////////////////////// Object //////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Object {
    ident: Identifier,
    number: FieldNumber,
    fields: Vec<FieldDefinition>,
}

impl Object {
    pub fn new(
        ident: Identifier,
        number: FieldNumber,
        fields: Vec<FieldDefinition>,
    ) -> Result<Self, Error> {
        check_fields(&fields)?;
        Ok(Self {
            ident,
            number,
            fields,
        })
    }

    pub fn parse<S: AsRef<str>>(object: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::object)(object.as_ref())?)
    }

    pub fn lookup_field(&self, ident: &Identifier) -> Result<&FieldDefinition, Error> {
        for field in self.fields.iter() {
            if field.ident() == ident {
                return Ok(field);
            }
        }
        Err(Error::UnknownField {
            core: ErrorCore::default(),
            ident: ident.to_string(),
        })
    }

    pub fn to_protoql(&self) -> String {
        let mut fields = self
            .fields
            .iter()
            .map(|f| f.to_protoql())
            .collect::<Vec<_>>()
            .join(";\n");
        if !fields.is_empty() {
            fields += ";";
        }
        let fields = indent(&fields);
        let ident = &self.ident;
        let number = self.number;
        format!(
            "object {ident} = {number} {{
{fields}
}}"
        )
    }

    fn to_protobuf(&self) -> String {
        let mut fields = self
            .fields
            .iter()
            .map(|f| f.to_protobuf())
            .collect::<Vec<_>>()
            .join(";\n");
        if !fields.is_empty() {
            fields = indent(&fields);
            fields += ";\n";
        }
        let message_type = self.ident.to_camel_case();
        let ident = &self.ident;
        let number = &self.number;
        format!(
            "message {message_type} {{
{fields}}};
optional {message_type} {ident} = {number}",
        )
    }

    pub fn describe_keys(&self, prefix: &str) -> Vec<String> {
        let mut keys = vec![];
        let mut prefix = prefix.to_string();
        prefix += &format!(", {}:{}:message", self.number, self.ident);
        keys.push(prefix.clone() + ")");
        for field in self.fields.iter() {
            keys.append(&mut field.describe_keys(&prefix));
        }
        keys
    }
}

//////////////////////////////////////////////// Map ///////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Map {
    key: Key,
    fields: Vec<FieldDefinition>,
}

impl Map {
    pub fn new(key: Key, fields: Vec<FieldDefinition>) -> Result<Self, Error> {
        check_fields(&fields)?;
        Ok(Self { key, fields })
    }

    pub fn parse<S: AsRef<str>>(map: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::map_field)(map.as_ref())?)
    }

    pub fn lookup_field(&self, ident: &Identifier) -> Result<&FieldDefinition, Error> {
        for field in self.fields.iter() {
            if field.ident() == ident {
                return Ok(field);
            }
        }
        Err(Error::UnknownField {
            core: ErrorCore::default(),
            ident: ident.to_string(),
        })
    }

    pub fn to_protoql(&self) -> String {
        let mut fields = self
            .fields
            .iter()
            .map(|f| f.to_protoql())
            .collect::<Vec<_>>()
            .join(";\n");
        if !fields.is_empty() {
            fields += ";";
        }
        let fields = indent(&fields);
        let ty = DataType::from(self.key.ty).to_protoql();
        let ident = self.key.ident.to_string();
        let number = self.key.number;
        format!(
            "map {ty} {ident} = {number} {{
{fields}
}}"
        )
    }

    fn to_protobuf(&self) -> String {
        let mut fields = self
            .fields
            .iter()
            .map(|f| f.to_protobuf())
            .collect::<Vec<_>>()
            .join(";\n");
        if !fields.is_empty() {
            fields = indent(&fields);
            fields += ";\n";
        }
        let message_type = self.key.ident.to_camel_case() + "Value";
        let key_type = DataType::from(self.key.ty).to_protobuf();
        let ident = self.key.ident.to_string();
        let number = self.key.number;
        format!(
            "message {message_type} {{
{fields}}}
map<{key_type}, {message_type}> {ident} = {number}"
        )
    }

    pub fn describe_keys(&self, prefix: &str) -> Vec<String> {
        let mut keys = vec![];
        let mut prefix = prefix.to_string();
        let key_type = DataType::from(self.key.ty).to_protobuf();
        prefix += &format!(", {}:{}:map<{}>", self.key.number, self.key.ident, key_type);
        keys.push(prefix.clone() + ")");
        for field in self.fields.iter() {
            keys.append(&mut field.describe_keys(&prefix));
        }
        keys
    }
}

/////////////////////////////////////////////// Join ///////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Join {
    ident: Identifier,
    number: FieldNumber,
    join_table: Identifier,
    join_keys: Vec<Identifier>,
}

impl Join {
    pub fn new(
        ident: Identifier,
        number: FieldNumber,
        join_table: Identifier,
        join_keys: Vec<Identifier>,
    ) -> Result<Self, Error> {
        Ok(Self {
            ident,
            number,
            join_table,
            join_keys,
        })
    }

    pub fn parse<S: AsRef<str>>(join: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::join)(join.as_ref())?)
    }

    pub fn to_protoql(&self) -> String {
        let ident = self.ident.to_string();
        let number = self.number;
        let join_table = self.join_table.to_string();
        let join_keys = self
            .join_keys
            .iter()
            .map(|k| k.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("join {ident} = {number} on {join_table} ({join_keys})")
    }

    fn to_protobuf(&self) -> String {
        let message_type = self.join_table.to_camel_case();
        let ident = self.ident.to_string();
        let number = self.number;
        format!("optional {message_type} {ident} = {number}")
    }

    pub fn describe_keys(&self, _: &str) -> Vec<String> {
        vec![]
    }
}

////////////////////////////////////////// FieldDefinition /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldDefinition {
    Field(Field),
    Object(Object),
    Map(Map),
    Join(Join),
}

impl FieldDefinition {
    pub fn parse<S: AsRef<str>>(fd: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::field_definition)(fd.as_ref())?)
    }

    pub fn ident(&self) -> &Identifier {
        match self {
            FieldDefinition::Field(field) => &field.ident,
            FieldDefinition::Object(object) => &object.ident,
            FieldDefinition::Map(map) => &map.key.ident,
            FieldDefinition::Join(join) => &join.ident,
        }
    }

    pub fn field_number(&self) -> FieldNumber {
        match self {
            FieldDefinition::Field(field) => field.number,
            FieldDefinition::Object(object) => object.number,
            FieldDefinition::Map(map) => map.key.number,
            FieldDefinition::Join(join) => join.number,
        }
    }

    pub fn to_protoql(&self) -> String {
        match self {
            FieldDefinition::Field(field) => field.to_protoql(),
            FieldDefinition::Object(object) => object.to_protoql(),
            FieldDefinition::Map(map) => map.to_protoql(),
            FieldDefinition::Join(join) => join.to_protoql(),
        }
    }

    fn to_protobuf(&self) -> String {
        match self {
            FieldDefinition::Field(field) => field.to_protobuf(),
            FieldDefinition::Object(object) => object.to_protobuf(),
            FieldDefinition::Map(map) => map.to_protobuf(),
            FieldDefinition::Join(join) => join.to_protobuf(),
        }
    }

    fn describe_keys(&self, prefix: &str) -> Vec<String> {
        match self {
            FieldDefinition::Field(field) => field.describe_keys(prefix),
            FieldDefinition::Object(object) => object.describe_keys(prefix),
            FieldDefinition::Map(map) => map.describe_keys(prefix),
            FieldDefinition::Join(join) => join.describe_keys(prefix),
        }
    }
}

/////////////////////////////////////////////// Table //////////////////////////////////////////////

#[derive(Debug, Eq, PartialEq)]
pub struct Table {
    ident: Identifier,
    number: FieldNumber,
    key: Vec<Key>,
    fields: Vec<FieldDefinition>,
}

impl Table {
    pub fn new(
        ident: Identifier,
        number: FieldNumber,
        key: Vec<Key>,
        fields: Vec<FieldDefinition>,
    ) -> Result<Self, Error> {
        check_key(&key)?;
        check_fields(&fields)?;
        for k in key.iter() {
            for f in fields.iter() {
                if k.ident == *f.ident() {
                    return Err(Error::DuplicateIdentifier {
                        core: ErrorCore::default(),
                        ident: k.ident.to_string(),
                    });
                }
                if k.number == f.field_number() {
                    return Err(Error::DuplicateFieldNumber {
                        core: ErrorCore::default(),
                        number: k.number.get(),
                    });
                }
            }
        }
        Ok(Table {
            ident,
            number,
            key,
            fields,
        })
    }

    pub fn parse<S: AsRef<str>>(table: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::table)(table.as_ref())?)
    }

    pub fn lookup_field(&self, ident: &Identifier) -> Result<FieldDefinition, Error> {
        for k in self.key.iter() {
            if k.ident == *ident {
                return Ok(FieldDefinition::Field(k.into()));
            }
        }
        for f in self.fields.iter() {
            if f.ident() == ident {
                return Ok(f.clone());
            }
        }
        Err(Error::UnknownField {
            core: ErrorCore::default(),
            ident: ident.to_string(),
        })
    }

    pub fn to_protoql(&self) -> String {
        let mut keys = String::new();
        for key in self.key.iter() {
            if !keys.is_empty() {
                keys += ", ";
            }
            keys += &key.to_protoql();
        }
        let mut fields = String::new();
        for field in self.fields.iter() {
            if !fields.is_empty() {
                fields += "\n";
            }
            fields += &indent(&field.to_protoql());
            fields += ";";
        }
        let ident = &self.ident;
        let number = self.number;
        format!(
            "table {ident} ({keys}) @ {number} {{
{fields}
}}"
        )
    }

    pub fn to_protobuf(&self) -> String {
        let mut fields = String::new();
        for key in self.key.iter() {
            if !fields.is_empty() {
                fields += "\n";
            }
            fields += &key.to_protobuf();
            fields += ";";
        }
        for field in self.fields.iter() {
            if !fields.is_empty() {
                fields += "\n";
            }
            fields += &field.to_protobuf();
            fields += ";";
        }
        if !fields.is_empty() {
            fields = indent(&fields);
        }
        let ident = self.ident.to_camel_case();
        format!(
            r"message {ident} {{
{fields}
}}",
        )
    }

    pub fn describe_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        let mut prefix = format!("({}:{}:TableID", self.number, self.ident.to_camel_case());
        for key in self.key.iter() {
            let ty: DataType = key.ty.into();
            prefix += &format!(", {}:{}:{}", key.number, key.ident, ty.to_protobuf());
        }
        keys.push(prefix.clone() + ")");
        for field in self.fields.iter() {
            keys.append(&mut field.describe_keys(&prefix));
        }
        keys
    }
}

///////////////////////////////////////////// TableSet /////////////////////////////////////////////

#[derive(Debug, Default, Eq, PartialEq)]
pub struct TableSet {
    tables: Vec<Table>,
}

impl TableSet {
    pub fn new(tables: Vec<Table>) -> Result<Self, Error> {
        check_tables(&tables)?;
        Ok(Self { tables })
    }

    pub fn parse<S: AsRef<str>>(table_set: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::table_set)(table_set.as_ref())?)
    }

    pub fn lookup_table(&self, ident: &Identifier) -> Result<&Table, Error> {
        for table in self.tables.iter() {
            if table.ident == *ident {
                return Ok(table);
            }
        }
        Err(Error::UnknownTable {
            core: ErrorCore::default(),
            ident: ident.to_string(),
        })
    }

    pub fn to_protoql(&self) -> String {
        let mut ret = String::new();
        for table in self.tables.iter() {
            if !ret.is_empty() {
                ret += "\n";
            }
            ret += &table.to_protoql();
            ret += "\n";
        }
        ret
    }

    pub fn to_protobuf(&self) -> String {
        let mut ret = "syntax = \"proto2\";\n\n".to_string();
        for table in self.tables.iter() {
            ret += &table.to_protobuf();
            ret += "\n\n";
        }
        ret
    }

    pub fn describe_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for table in self.tables.iter() {
            keys.append(&mut table.describe_keys());
        }
        keys
    }
}

impl From<Table> for TableSet {
    fn from(table: Table) -> Self {
        let tables = vec![table];
        Self { tables }
    }
}

////////////////////////////////////////////// Schema //////////////////////////////////////////////

// NOTE(rescrv): This is inefficient for simplicity's sake.  Make it correct with tests, then fast.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Schema {
    entries: Vec<SchemaEntry>,
}

impl Schema {
    fn new() -> Self {
        Self {
            entries: vec![SchemaEntry {
                key: SchemaKey::default(),
                value: DataType::message,
            }],
        }
    }

    #[allow(dead_code)]
    fn add_to_schema(&mut self, entry: SchemaEntry) -> Result<(), Error> {
        let mut prefixes = Vec::new();
        prefixes.push(entry.clone());
        while let Some(back) = prefixes[prefixes.len() - 1].prefix() {
            prefixes.push(back);
        }
        let mut register = Vec::new();
        while let Some(back) = prefixes.pop() {
            let mut found = false;
            for entry in self.entries.iter() {
                back.check_compatibility(entry)?;
                if back == *entry {
                    found = true;
                }
            }
            if !found {
                register.push(back);
            }
        }
        self.entries.append(&mut register);
        self.entries.sort();
        self.check_self_compatible()?;
        Ok(())
    }

    fn lookup_schema_for_key(&self, key: &[u8]) -> Result<Option<&SchemaEntry>, Error> {
        let mut tki = TupleKeyIterator::from(key);
        let mut fields = Vec::new();
        'looping: loop {
            let tag = match tki.next() {
                Some(tag) => tag,
                None => {
                    break 'looping;
                }
            };
            let _ = match tki.next() {
                Some(value) => value,
                None => {
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        what: "tuple key should always have fields in pairs".to_owned(),
                    });
                }
            };
            fn to_field_number(buf: &[u8]) -> Result<(FieldNumber, KeyDataType, Direction), Error> {
                let mut copy = [0u8; 10];
                let sz = std::cmp::min(buf.len(), copy.len());
                for (c, b) in std::iter::zip(&mut copy[..sz], &buf[..sz]) {
                    *c = b.rotate_right(1);
                }
                let x: v64 = v64::unpack(&copy[..sz])
                    .map_err(|err| Error::Corruption {
                        core: ErrorCore::default(),
                        what: format!("unparseable field number: {err}"),
                    })
                    .as_z()
                    .with_info("bytes", &copy[..sz])?
                    .0;
                let x: u64 = x.into();
                if x >> 4 >= u32::max_value() as u64 {
                    return Err(Error::Corruption {
                        core: ErrorCore::default(),
                        what: "invalid field number".to_owned(),
                    })
                    .as_z()
                    .with_info("x", x);
                }
                let f = FieldNumber::new((x >> 4) as u32)
                    .map_err(|err| Error::Corruption {
                        core: ErrorCore::default(),
                        what: format!("invalid field number: {err}"),
                    })
                    .as_z()
                    .with_info("field number", x >> 4)?;
                let (v, d) = match tuple_key::from_discriminant(x as u8 & 15) {
                    Some((v, d)) => (v, d),
                    None => {
                        return Err(Error::Corruption {
                            core: ErrorCore::default(),
                            what: "invalid discriminant".to_owned(),
                        })
                        .as_z()
                        .with_info("discriminant", x & 15);
                    }
                };
                Ok((f, v, d))
            }
            let (number, ty, dir) = to_field_number(tag)?;
            fields.push(SchemaKeyElement { number, ty, dir });
        }
        for idx in 0..self.entries.len() {
            if self.entries[idx].key.matches_elements(&fields) {
                return Ok(Some(&self.entries[idx]));
            }
        }
        Ok(None)
    }

    fn check_self_compatible(&self) -> Result<(), Error> {
        for entry_lhs in self.entries.iter() {
            for entry_rhs in self.entries.iter() {
                entry_lhs.check_compatibility(entry_rhs)?;
            }
        }
        Ok(())
    }

    pub fn check_compatibility(&self, other: &Self) -> Result<(), Error> {
        self.check_self_compatible()?;
        other.check_self_compatible()?;
        for entry_lhs in self.entries.iter() {
            for entry_rhs in other.entries.iter() {
                entry_lhs.check_compatibility(entry_rhs)?;
            }
        }
        Ok(())
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}

//////////////////////////////////////////// SchemaEntry ///////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct SchemaEntry {
    key: SchemaKey,
    value: DataType,
}

impl SchemaEntry {
    #[allow(dead_code)]
    fn new(key: SchemaKey, value: DataType) -> Self {
        Self { key, value }
    }

    fn key(&self) -> &SchemaKey {
        &self.key
    }

    fn value(&self) -> DataType {
        self.value
    }

    fn is_extendable_by(&self, other: &Self) -> bool {
        #[allow(clippy::comparison_chain)]
        if self.key.elements.len() < other.key.elements.len() {
            self.key.elements == other.key.elements[..self.key.elements.len()]
                && self.value == DataType::message
        } else if self.key.elements.len() == other.key.elements.len() {
            self.key.elements == other.key.elements[..self.key.elements.len()]
                && self.value == other.value
        } else {
            false
        }
    }

    fn pop_field(&mut self) {
        if !self.key.elements.is_empty() {
            self.key.elements.pop();
            self.value = DataType::message;
        }
    }

    fn push_field(&mut self, field: SchemaKeyElement, value: DataType) {
        assert_eq!(DataType::message, self.value);
        self.key.elements.push(field);
        self.value = value;
    }

    fn check_compatibility(&self, other: &Self) -> Result<(), Error> {
        let mut breaked = false;
        for (idx, (lhs, rhs)) in
            std::iter::zip(self.key.elements.iter(), other.key.elements.iter()).enumerate()
        {
            if lhs.number == rhs.number && lhs.ty != rhs.ty {
                return Err(Error::SchemaIncompatibility {
                    core: ErrorCore::default(),
                    what: "field number same; type different".to_owned(),
                })
                .as_z()
                .with_info("index", idx)
                .with_info("lhs.number", lhs.number)
                .with_info("rhs.number", rhs.number)
                .with_info("lhs.ty", lhs.ty)
                .with_info("rhs.ty", rhs.ty);
            }
            if lhs.number != rhs.number {
                breaked = true;
                break;
            }
        }
        if !breaked {
            if self.key.elements.len() < other.key.elements.len() && self.value != DataType::message
            {
                return Err(Error::SchemaIncompatibility {
                    core: ErrorCore::default(),
                    what: "lhs has non-message type and is prefix of rhs".to_owned(),
                })
                .as_z()
                .with_info("lhs.ty", self.value);
            }
            if self.key.elements.len() > other.key.elements.len()
                && other.value != DataType::message
            {
                return Err(Error::SchemaIncompatibility {
                    core: ErrorCore::default(),
                    what: "rhs has non-message type and is prefix of lhs".to_owned(),
                })
                .as_z()
                .with_info("rhs.ty", other.value);
            }
            if self.key.elements == other.key.elements && self.value != other.value {
                return Err(Error::SchemaIncompatibility {
                    core: ErrorCore::default(),
                    what: "lhs and rhs have same fields, but different values".to_owned(),
                })
                .as_z()
                .with_info("lhs.value", self.value)
                .with_info("rhs.value", other.value);
            }
        }
        Ok(())
    }

    fn prefix(&self) -> Option<Self> {
        if self.key.elements.is_empty() {
            return None;
        }
        let mut fields = self.key.elements.clone();
        fields.pop();
        Some(SchemaEntry {
            key: SchemaKey::new(fields),
            value: DataType::message,
        })
    }
}

impl Default for SchemaEntry {
    fn default() -> Self {
        Self {
            key: SchemaKey::default(),
            value: DataType::message,
        }
    }
}

///////////////////////////////////////////// SchemaKey ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct SchemaKey {
    elements: Vec<SchemaKeyElement>,
}

impl SchemaKey {
    fn new(elements: Vec<SchemaKeyElement>) -> Self {
        Self { elements }
    }

    fn matches_elements(&self, elements: &[SchemaKeyElement]) -> bool {
        if self.elements.len() != elements.len() {
            false
        } else {
            for (lhs, rhs) in std::iter::zip(self.elements.iter(), elements.iter()) {
                if lhs.number != rhs.number
                    || !DataType::can_cast(DataType::from(lhs.ty()), DataType::from(rhs.ty()))
                {
                    return false;
                }
            }
            true
        }
    }

    fn elements(&self) -> &[SchemaKeyElement] {
        &self.elements
    }
}

///////////////////////////////////////// SchemaKeyElement /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct SchemaKeyElement {
    number: FieldNumber,
    ty: KeyDataType,
    dir: Direction,
}

impl SchemaKeyElement {
    #[allow(dead_code)]
    fn new(number: FieldNumber, ty: KeyDataType, dir: Direction) -> Self {
        Self { number, ty, dir }
    }

    fn number(&self) -> FieldNumber {
        self.number
    }

    fn ty(&self) -> KeyDataType {
        self.ty
    }
}

impl From<&Key> for SchemaKeyElement {
    fn from(k: &Key) -> Self {
        Self {
            number: k.number,
            ty: k.ty,
            dir: k.dir,
        }
    }
}

impl From<&Field> for SchemaKeyElement {
    fn from(f: &Field) -> Self {
        Self {
            number: f.number,
            ty: KeyDataType::unit,
            dir: Direction::Forward,
        }
    }
}

impl From<&Object> for SchemaKeyElement {
    fn from(o: &Object) -> Self {
        Self {
            number: o.number,
            ty: KeyDataType::unit,
            dir: Direction::Forward,
        }
    }
}

impl From<&Map> for SchemaKeyElement {
    fn from(m: &Map) -> Self {
        Self {
            number: m.key.number,
            ty: m.key.ty,
            dir: m.key.dir,
        }
    }
}

/////////////////////////////////////////////// Query //////////////////////////////////////////////

#[derive(Debug, Eq, PartialEq)]
pub struct Query {
    ident: Identifier,
    filter: Option<QueryFilter>,
    exprs: Vec<Query>,
}

impl Query {
    pub fn new(ident: Identifier) -> Result<Self, Error> {
        Ok(Query {
            ident,
            filter: None,
            exprs: vec![],
        })
    }

    pub fn from_exprs(ident: Identifier, exprs: Vec<Query>) -> Result<Self, Error> {
        Ok(Query {
            ident,
            filter: None,
            exprs,
        })
    }

    pub fn from_filter(ident: Identifier, filter: QueryFilter) -> Result<Self, Error> {
        Ok(Query {
            ident,
            filter: Some(filter),
            exprs: vec![],
        })
    }

    pub fn from_filter_and_exprs(
        ident: Identifier,
        filter: QueryFilter,
        exprs: Vec<Query>,
    ) -> Result<Self, Error> {
        Ok(Query {
            ident,
            filter: Some(filter),
            exprs,
        })
    }

    pub fn parse<S: AsRef<str>>(query: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::query)(query.as_ref())?)
    }
}

//////////////////////////////////////////// QueryFilter ///////////////////////////////////////////

#[derive(Debug, Eq, PartialEq)]
pub enum QueryFilter {
    Equals(KeyLiteral),
}

/////////////////////////////////////////////// utils //////////////////////////////////////////////

pub fn check_key(_: &[Key]) -> Result<(), Error> {
    Ok(())
}

pub fn check_fields(fields: &[FieldDefinition]) -> Result<(), Error> {
    for i in 0..fields.len() {
        for j in i + 1..fields.len() {
            if fields[i].ident() == fields[j].ident() {
                return Err(Error::DuplicateIdentifier {
                    core: ErrorCore::default(),
                    ident: fields[i].ident().to_string(),
                });
            }
            if fields[i].field_number() == fields[j].field_number() {
                return Err(Error::DuplicateFieldNumber {
                    core: ErrorCore::default(),
                    number: fields[i].field_number().get(),
                });
            }
        }
    }
    Ok(())
}

pub fn check_table(_: &Table) -> Result<(), Error> {
    Ok(())
}

pub fn check_tables(tables: &[Table]) -> Result<(), Error> {
    for i in 0..tables.len() {
        for j in i + 1..tables.len() {
            if tables[i].ident == tables[j].ident {
                return Err(Error::DuplicateIdentifier {
                    core: ErrorCore::default(),
                    ident: tables[i].ident.to_string(),
                });
            }
            if tables[i].number == tables[j].number {
                return Err(Error::DuplicateFieldNumber {
                    core: ErrorCore::default(),
                    number: tables[i].number.get(),
                });
            }
        }
    }
    // TODO(rescrv):  Check joins.
    for table in tables {
        check_table(table)?;
    }
    Ok(())
}

/////////////////////////////////////////////// utils //////////////////////////////////////////////

fn indent(s: &str) -> String {
    let s: Vec<String> = s.split('\n').map(|s| "    ".to_string() + s).collect();
    s.join("\n")
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod test {
    use std::io::Write;
    use std::process::Command;

    use super::*;

    #[test]
    fn some_table() {
        let table = Table::parse(
            "table table_name (string some_key=9) @ 5 {
            uint64 value1 = 2;
            string public_image = 3;
            breakout uint64 last_seen = 4;
        }",
        )
        .expect("parsing should never fail");
        assert_eq!(
            Table {
                ident: Identifier::must("table_name"),
                number: FieldNumber::must(5),
                key: vec![Key {
                    ident: Identifier::must("some_key"),
                    number: FieldNumber::must(9),
                    ty: KeyDataType::string,
                    dir: Direction::Forward,
                },],
                fields: vec![
                    FieldDefinition::Field(Field {
                        ident: Identifier::must("value1"),
                        number: FieldNumber::must(2),
                        ty: DataType::uint64,
                        breakout: false,
                    }),
                    FieldDefinition::Field(Field {
                        ident: Identifier::must("public_image"),
                        number: FieldNumber::must(3),
                        ty: DataType::string,
                        breakout: false,
                    }),
                    FieldDefinition::Field(Field {
                        ident: Identifier::must("last_seen"),
                        number: FieldNumber::must(4),
                        ty: DataType::uint64,
                        breakout: true,
                    }),
                ],
            },
            table
        );
    }

    mod identifier {
        use super::Identifier;

        #[test]
        #[should_panic]
        fn empty_string() {
            let _ident = Identifier::must("");
        }

        #[test]
        fn identifier9() {
            let ident = Identifier::must("__identifier9");
            assert_eq!("__identifier9", ident.ident);
        }
    }

    mod field {
        use super::{DataType, Field, FieldNumber, Identifier};

        #[test]
        fn bytes16_number_9() {
            let field = Field::parse("bytes16 some_key = 9").unwrap();
            assert_eq!(
                Field {
                    ident: Identifier::must("some_key"),
                    number: FieldNumber::must(9),
                    ty: DataType::bytes16,
                    breakout: false,
                },
                field
            );
        }

        #[test]
        fn min_spaces() {
            let field = Field::parse("bytes16 some_key=9").unwrap();
            assert_eq!(
                Field {
                    ident: Identifier::must("some_key"),
                    number: FieldNumber::must(9),
                    ty: DataType::bytes16,
                    breakout: false,
                },
                field
            );
        }
    }

    macro_rules! test_per_file {
        ($name:ident { $($func:ident => $s:expr,)+ }) => {
            $(
                #[test]
                fn $func() {
                    $name(file!(), line!(), $s);
                }
            )+
        }
    }

    fn test_path(test_data: &str, suffix: &str) -> String {
        "protoqltests/".to_string() + test_data + suffix
    }

    fn diff_path(path: &str, data: &str) {
        let data = data.trim_end();
        if std::fs::read_to_string(path).expect("filesystem should always work") != data {
            let mut child = Command::new("diff")
                .arg("-u")
                .arg("-w")
                .arg(path)
                .arg("/dev/stdin")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("diff should never fail");
            if let Some(stdin) = child.stdin.as_mut() {
                stdin
                    .write_all(data.as_ref())
                    .expect("pipe write should never fail");
            }
            let output = child.wait_with_output().expect("diff should never fail");
            if output.status.success() {
            } else {
                panic!(
                    "different test case {}:\n{}",
                    path,
                    String::from_utf8_lossy(&output.stdout)
                );
            }
        }
    }

    mod table_set {
        use crate::TableSet;

        use super::{diff_path, test_path};

        fn table_set_example(_: &'static str, _: u32, test_data: &str) {
            let table_set_path = test_path(test_data, ".schema.protoql");
            let protoql_table_set =
                std::fs::read_to_string(table_set_path).expect("filesystem should always work");
            let table_set =
                TableSet::parse(protoql_table_set).expect("test table_set should always parse");
            diff_path(
                &test_path(test_data, ".schema.describe"),
                &format!("{:#?}", table_set),
            );
            diff_path(
                &test_path(test_data, ".schema.protoc"),
                &table_set.to_protobuf(),
            );
        }

        test_per_file! {
            table_set_example {
                user_account => "user_account",
            }
        }
    }

    mod keyed_schema {
        use super::*;

        #[test]
        fn empty_schema_is_compatible() {
            let schema1 = Schema::default();
            let schema2 = Schema::default();
            schema1.check_compatibility(&schema2).unwrap();
        }

        #[test]
        fn compatible_schemas() {
            let mut schema1 = Schema::default();
            let mut schema2 = Schema::default();
            schema1
                .add_to_schema(SchemaEntry {
                    key: SchemaKey {
                        elements: vec![
                            SchemaKeyElement {
                                number: FieldNumber::must(1),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(2),
                                ty: KeyDataType::unit,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(3),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                        ],
                    },
                    value: DataType::uint64,
                })
                .unwrap();
            schema2
                .add_to_schema(SchemaEntry {
                    key: SchemaKey {
                        elements: vec![
                            SchemaKeyElement {
                                number: FieldNumber::must(1),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(2),
                                ty: KeyDataType::unit,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(4),
                                ty: KeyDataType::fixed64,
                                dir: Direction::Forward,
                            },
                        ],
                    },
                    value: DataType::uint64,
                })
                .unwrap();
            schema1.check_compatibility(&schema2).unwrap();
        }

        #[test]
        fn incompatible_schemas_prefix() {
            let mut schema1 = Schema::default();
            let mut schema2 = Schema::default();
            schema1
                .add_to_schema(SchemaEntry {
                    key: SchemaKey {
                        elements: vec![
                            SchemaKeyElement {
                                number: FieldNumber::must(1),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(2),
                                ty: KeyDataType::unit,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(3),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                        ],
                    },
                    value: DataType::uint64,
                })
                .unwrap();
            schema2
                .add_to_schema(SchemaEntry {
                    key: SchemaKey {
                        elements: vec![
                            SchemaKeyElement {
                                number: FieldNumber::must(1),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(2),
                                ty: KeyDataType::fixed64,
                                dir: Direction::Forward,
                            },
                        ],
                    },
                    value: DataType::uint64,
                })
                .unwrap();
            if let Err(err) = schema1.check_compatibility(&schema2) {
                assert_eq!(
                    "SchemaIncompatibility { what: \"field number same; type different\" }",
                    err.to_string()
                );
            } else {
                panic!();
            }
        }

        #[test]
        fn incompatible_schemas_value() {
            let mut schema1 = Schema::default();
            let mut schema2 = Schema::default();
            schema1
                .add_to_schema(SchemaEntry {
                    key: SchemaKey {
                        elements: vec![
                            SchemaKeyElement {
                                number: FieldNumber::must(1),
                                ty: KeyDataType::fixed64,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(2),
                                ty: KeyDataType::unit,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(3),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                        ],
                    },
                    value: DataType::sint64,
                })
                .unwrap();
            schema2
                .add_to_schema(SchemaEntry {
                    key: SchemaKey {
                        elements: vec![
                            SchemaKeyElement {
                                number: FieldNumber::must(1),
                                ty: KeyDataType::fixed64,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(2),
                                ty: KeyDataType::unit,
                                dir: Direction::Forward,
                            },
                            SchemaKeyElement {
                                number: FieldNumber::must(3),
                                ty: KeyDataType::string,
                                dir: Direction::Forward,
                            },
                        ],
                    },
                    value: DataType::int64,
                })
                .unwrap();
            if let Err(err) = schema1.check_compatibility(&schema2) {
                assert_eq!("SchemaIncompatibility { what: \"lhs and rhs have same fields, but different values\" }", err.to_string());
            } else {
                panic!();
            }
        }

        #[test]
        fn lookup_schema_for_key1() {
            let mut schema = Schema::default();
            let entry = SchemaEntry {
                key: SchemaKey {
                    elements: vec![
                        SchemaKeyElement {
                            number: FieldNumber::must(1),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(2),
                            ty: KeyDataType::unit,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(3),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                    ],
                },
                value: DataType::uint64,
            };
            schema.add_to_schema(entry.clone()).unwrap();
            let mut tk = TupleKey::default();
            tk.extend_with_key(
                FieldNumber::must(1),
                "Element 1".to_owned(),
                Direction::Forward,
            );
            tk.extend(FieldNumber::must(2));
            tk.extend_with_key(
                FieldNumber::must(3),
                "Element 3".to_owned(),
                Direction::Forward,
            );
            assert_eq!(
                Ok(Some(&entry)),
                schema.lookup_schema_for_key(tk.as_bytes())
            );
        }

        #[test]
        fn lookup_schema_for_key_cast() {
            let mut schema = Schema::default();
            let entry = SchemaEntry {
                key: SchemaKey {
                    elements: vec![SchemaKeyElement {
                        number: FieldNumber::must(1),
                        ty: KeyDataType::fixed64,
                        dir: Direction::Forward,
                    }],
                },
                value: DataType::uint64,
            };
            schema.add_to_schema(entry.clone()).unwrap();
            let mut tk = TupleKey::default();
            tk.extend_with_key(FieldNumber::must(1), 42u64, Direction::Forward);
            assert_eq!(
                Ok(Some(&entry)),
                schema.lookup_schema_for_key(tk.as_bytes())
            );
        }

        #[test]
        fn lookup_schema_for_key_not_found() {
            let mut schema = Schema::default();
            let entry = SchemaEntry {
                key: SchemaKey {
                    elements: vec![
                        SchemaKeyElement {
                            number: FieldNumber::must(1),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(2),
                            ty: KeyDataType::unit,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(3),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                    ],
                },
                value: DataType::uint64,
            };
            schema.add_to_schema(entry.clone()).unwrap();
            let mut tk = TupleKey::default();
            tk.extend_with_key(
                FieldNumber::must(2),
                "Element 1".to_owned(),
                Direction::Forward,
            );
            tk.extend(FieldNumber::must(2));
            tk.extend_with_key(
                FieldNumber::must(4),
                "Element 3".to_owned(),
                Direction::Forward,
            );
            assert_eq!(Ok(None), schema.lookup_schema_for_key(tk.as_bytes()));
        }

        #[test]
        fn schema_entry_extends() {
            let mut entry1 = SchemaEntry {
                key: SchemaKey {
                    elements: vec![
                        SchemaKeyElement {
                            number: FieldNumber::must(1),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(2),
                            ty: KeyDataType::unit,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(3),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                    ],
                },
                value: DataType::message,
            };
            let entry2 = SchemaEntry {
                key: SchemaKey {
                    elements: vec![
                        SchemaKeyElement {
                            number: FieldNumber::must(1),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(2),
                            ty: KeyDataType::unit,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(3),
                            ty: KeyDataType::string,
                            dir: Direction::Forward,
                        },
                        SchemaKeyElement {
                            number: FieldNumber::must(4),
                            ty: KeyDataType::unit,
                            dir: Direction::Forward,
                        },
                    ],
                },
                value: DataType::uint64,
            };

            assert!(entry1.is_extendable_by(&entry1));
            assert!(entry1.is_extendable_by(&entry2));
            assert!(!entry2.is_extendable_by(&entry1));
            entry1.value = DataType::string;
            assert!(!entry1.is_extendable_by(&entry2));
        }
    }
}
