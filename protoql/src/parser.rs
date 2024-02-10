use std::fmt::{Formatter, Write};

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag},
    character::complete::{
        self as character, alpha1, alphanumeric1, digit1, multispace0, multispace1, none_of, one_of,
    },
    combinator::{all_consuming, cut, map, map_res, opt, recognize},
    error::{context, VerboseError, VerboseErrorKind},
    multi::{many0, many0_count, separated_list0, separated_list1},
    sequence::{delimited, pair, terminated, tuple},
    IResult, Offset,
};

use prototk::FieldNumber;
use zerror_core::ErrorCore;

use crate::{
    DataType, Direction, Error, Field, FieldDefinition, Identifier, Join, Key, KeyDataType,
    KeyLiteral, Map, Object, Query, QueryFilter, Table, TableSet,
};

////////////////////////////////////////// error handling //////////////////////////////////////////

type ParseResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

#[derive(Clone, Eq, PartialEq)]
pub struct ParseError {
    string: String,
}

impl std::fmt::Debug for ParseError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        writeln!(fmt, "{}", self.string)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        writeln!(fmt, "{}", self.string)
    }
}

impl From<String> for ParseError {
    fn from(string: String) -> Self {
        Self { string }
    }
}

pub fn interpret_verbose_error(input: &'_ str, err: VerboseError<&'_ str>) -> ParseError {
    let mut result = String::new();
    let mut index = 0;
    for (substring, kind) in err.errors.iter() {
        let offset = input.offset(substring);
        let prefix = &input.as_bytes()[..offset];
        // Count the number of newlines in the first `offset` bytes of input
        let line_number = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
        // Find the line that includes the subslice:
        // Find the *last* newline before the substring starts
        let line_begin = prefix
            .iter()
            .rev()
            .position(|&b| b == b'\n')
            .map(|pos| offset - pos)
            .unwrap_or(0);
        // Find the full line after that newline
        let line = input[line_begin..]
            .lines()
            .next()
            .unwrap_or(&input[line_begin..])
            .trim_end();
        // The (1-indexed) column number is the offset of our substring into that line
        let column_number = line.offset(substring) + 1;
        match kind {
            VerboseErrorKind::Char(c) => {
                if let Some(actual) = substring.chars().next() {
                    write!(
                        &mut result,
                        "{index}: at line {line_number}:\n\
                 {line}\n\
                 {caret:>column$}\n\
                 expected '{expected}', found {actual}\n\n",
                        index = index,
                        line_number = line_number,
                        line = line,
                        caret = '^',
                        column = column_number,
                        expected = c,
                        actual = actual,
                    )
                    .unwrap();
                } else {
                    write!(
                        &mut result,
                        "{index}: at line {line_number}:\n\
                 {line}\n\
                 {caret:>column$}\n\
                 expected '{expected}', got end of input\n\n",
                        index = index,
                        line_number = line_number,
                        line = line,
                        caret = '^',
                        column = column_number,
                        expected = c,
                    )
                    .unwrap();
                }
                index += 1;
            }
            VerboseErrorKind::Context(s) => {
                write!(
                    &mut result,
                    "{index}: at line {line_number}, in {context}:\n\
               {line}\n\
               {caret:>column$}\n\n",
                    index = index,
                    line_number = line_number,
                    context = s,
                    line = line,
                    caret = '^',
                    column = column_number,
                )
                .unwrap();
                index += 1;
            }
            // Swallow these.   They are ugly.
            VerboseErrorKind::Nom(_) => {}
        };
    }
    ParseError {
        string: result.trim().to_string(),
    }
}

///////////////////////////////////////////// TableSet /////////////////////////////////////////////

pub fn identifier(input: &str) -> ParseResult<Identifier> {
    context(
        "identifier",
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                many0_count(alt((alphanumeric1, tag("_")))),
            )),
            |ident: &str| Identifier {
                ident: ident.to_string(),
            },
        ),
    )(input)
}

pub fn identifier_list(input: &str) -> ParseResult<Vec<Identifier>> {
    context(
        "identifier list",
        terminated(
            separated_list1(
                tag(","),
                map(tuple((ws0, identifier, ws0)), |(_, ident, _)| ident),
            ),
            opt(tag(",")),
        ),
    )(input)
}

pub fn key_data_type(input: &str) -> ParseResult<KeyDataType> {
    let (input, recognized) = context(
        "key data type",
        alt((
            tag("unit"),
            tag("fixed32"),
            tag("fixed64"),
            tag("sfixed32"),
            tag("sfixed64"),
            tag("string"),
        )),
    )(input)?;
    Ok((
        input,
        match recognized {
            "unit" => KeyDataType::unit,
            "fixed32" => KeyDataType::fixed32,
            "fixed64" => KeyDataType::fixed64,
            "sfixed32" => KeyDataType::sfixed32,
            "sfixed64" => KeyDataType::sfixed64,
            "string" => KeyDataType::string,
            _ => {
                panic!("logic error");
            }
        },
    ))
}

pub fn data_type(input: &str) -> ParseResult<DataType> {
    let (input, recognized) = context(
        "data type",
        alt((
            tag("unit"),
            tag("int32"),
            tag("int64"),
            tag("uint32"),
            tag("uint64"),
            tag("sint32"),
            tag("sint64"),
            tag("fixed32"),
            tag("fixed64"),
            tag("sfixed32"),
            tag("sfixed64"),
            tag("timestamp_micros"),
            tag("float"),
            tag("double"),
            tag("bool"),
            tag("bytes16"),
            tag("bytes32"),
            tag("bytes64"),
            tag("bytes"),
            tag("string"),
            tag("message"),
        )),
    )(input)?;
    Ok((
        input,
        match recognized {
            "unit" => DataType::unit,
            "int32" => DataType::int32,
            "int64" => DataType::int64,
            "uint32" => DataType::uint32,
            "uint64" => DataType::uint64,
            "sint32" => DataType::sint32,
            "sint64" => DataType::sint64,
            "fixed32" => DataType::fixed32,
            "fixed64" => DataType::fixed64,
            "sfixed32" => DataType::sfixed32,
            "sfixed64" => DataType::sfixed64,
            "timestamp_micros" => DataType::timestamp_micros,
            "float" => DataType::float,
            "double" => DataType::double,
            "bool" => DataType::Bool,
            "bytes" => DataType::bytes,
            "bytes16" => DataType::bytes16,
            "bytes32" => DataType::bytes32,
            "bytes64" => DataType::bytes64,
            "string" => DataType::string,
            "message" => DataType::message,
            _ => {
                panic!("logic error");
            }
        },
    ))
}

pub fn field_number(input: &str) -> ParseResult<FieldNumber> {
    context("field number", map_res(character::u32, FieldNumber::new))(input)
}

pub fn key(input: &str) -> ParseResult<Key> {
    context(
        "key type",
        map_res(
            tuple((
                ws0,
                key_data_type,
                ws1,
                identifier,
                ws0,
                tag("="),
                ws0,
                field_number,
            )),
            |(_, ty, _, ident, _, _, _, number)| Key::new(ident, number, ty, Direction::Forward),
        ),
    )(input)
}

pub fn field(input: &str) -> ParseResult<Field> {
    context(
        "field type",
        alt((
            map_res(
                tuple((
                    ws0,
                    context("breakout", tag("breakout")),
                    ws1,
                    cut(data_type),
                    ws1,
                    identifier,
                    ws0,
                    tag("="),
                    ws0,
                    field_number,
                )),
                |(_, _, _, ty, _, ident, _, _, _, number)| Field::new(ident, number, ty, true),
            ),
            map_res(
                tuple((
                    ws0,
                    data_type,
                    ws1,
                    identifier,
                    ws0,
                    tag("="),
                    ws0,
                    field_number,
                )),
                |(_, ty, _, ident, _, _, _, number)| Field::new(ident, number, ty, false),
            ),
        )),
    )(input)
}

pub fn object(input: &str) -> ParseResult<Object> {
    context(
        "object type",
        map_res(
            tuple((
                ws0,
                tag("object"),
                cut(ws1),
                cut(identifier),
                cut(ws0),
                cut(tag("=")),
                cut(ws0),
                cut(field_number),
                cut(ws0),
                cut(tag("{")),
                cut(ws0),
                cut(table_field_list),
                cut(ws0),
                cut(tag("}")),
            )),
            |(_, _, _, ident, _, _, _, number, _, _, _, fields, _, _)| {
                Object::new(ident, number, fields)
            },
        ),
    )(input)
}

pub fn map_field(input: &str) -> ParseResult<Map> {
    context(
        "map type",
        map_res(
            tuple((
                ws0,
                tag("map"),
                cut(ws1),
                cut(key),
                cut(ws0),
                cut(tag("{")),
                cut(ws0),
                cut(table_field_list),
                cut(ws0),
                cut(tag("}")),
            )),
            |(_, _, _, key, _, _, _, fields, _, _)| Map::new(key, fields),
        ),
    )(input)
}

pub fn join(input: &str) -> ParseResult<Join> {
    context(
        "join field",
        map_res(
            tuple((
                ws0,
                tag("join"),
                cut(ws1),
                cut(identifier),
                cut(ws0),
                cut(tag("=")),
                cut(ws0),
                cut(field_number),
                cut(ws1),
                cut(tag("on")),
                cut(ws1),
                cut(identifier),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                cut(identifier_list),
                cut(ws0),
                cut(tag(")")),
            )),
            |(_, _, _, key, _, _, _, number, _, _, _, table, _, _, _, idents, _, _)| {
                Join::new(key, number, table, idents)
            },
        ),
    )(input)
}

pub fn field_definition(input: &str) -> ParseResult<FieldDefinition> {
    context(
        "field definition",
        alt((
            map(field, FieldDefinition::Field),
            map(object, FieldDefinition::Object),
            map(map_field, FieldDefinition::Map),
            map(join, FieldDefinition::Join),
        )),
    )(input)
}

pub fn key_field_list(input: &str) -> ParseResult<Vec<Key>> {
    context(
        "key fields",
        terminated(
            separated_list0(
                tag(","),
                map_res(
                    tuple((
                        ws0,
                        key_data_type,
                        cut(ws1),
                        cut(identifier),
                        cut(ws0),
                        cut(tag("=")),
                        cut(ws0),
                        cut(field_number),
                    )),
                    |(_, ty, _, ident, _, _, _, number)| {
                        Key::new(ident, number, ty, Direction::Forward)
                    },
                ),
            ),
            opt(tag(",")),
        ),
    )(input)
}

pub fn table_field_list(input: &str) -> ParseResult<Vec<FieldDefinition>> {
    context(
        "fields",
        map(
            many0(tuple((ws0, field_definition, ws0, tag(";")))),
            |fmjs: Vec<((), FieldDefinition, (), &str)>| {
                fmjs.into_iter().map(|f| f.1).collect::<Vec<_>>()
            },
        ),
    )(input)
}

pub fn table(input: &str) -> ParseResult<Table> {
    context(
        "table definition",
        map_res(
            tuple((
                ws0,
                context("table keyword", tag("table")),
                cut(ws1),
                context("table identifier", cut(identifier)),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                cut(key_field_list),
                cut(ws0),
                cut(tag(")")),
                cut(ws0),
                cut(tag("@")),
                cut(ws0),
                context("table number", cut(field_number)),
                cut(ws0),
                context("opening brace", cut(tag("{"))),
                cut(ws0),
                context("table body", cut(table_field_list)),
                cut(ws0),
                context("closing brace", cut(tag("}"))),
                cut(ws0),
            )),
            |(_, _, _, ident, _, _, _, key, _, _, _, _, _, number, _, _, _, fields, _, _, _)| {
                Table::new(ident, number, key, fields)
            },
        ),
    )(input)
}

pub fn table_set(input: &str) -> ParseResult<TableSet> {
    context(
        "table set",
        map_res(
            many0(alt((map(tuple((ws0, table, ws0)), |(_, t, _)| t),))),
            TableSet::new,
        ),
    )(input)
}

/////////////////////////////////////////////// Query //////////////////////////////////////////////

pub fn unescape(input: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    let mut prev_was_escape = false;
    for c in input.chars() {
        if prev_was_escape && (c == '\"' || c == '\\') {
            out.push(c);
            prev_was_escape = false;
        } else if c == '\\' {
            prev_was_escape = true;
        } else {
            out.push(c);
        }
    }
    out.into_iter().collect()
}

pub fn string_literal(input: &str) -> ParseResult<KeyLiteral> {
    context(
        "string literal",
        map(
            delimited(
                tag("\""),
                cut(alt((
                    escaped(none_of(r#"\""#), '\\', one_of(r#"\""#)),
                    tag(""),
                ))),
                tag("\""),
            ),
            |x: &str| KeyLiteral::string { value: unescape(x) },
        ),
    )(input)
}

fn number_to_typed(input: &str) -> Result<KeyLiteral, Error> {
    if let Ok(x) = str::parse::<i32>(input) {
        Ok(KeyLiteral::sfixed32 { value: x })
    } else if let Ok(x) = str::parse::<u32>(input) {
        Ok(KeyLiteral::fixed32 { value: x })
    } else if let Ok(x) = str::parse::<i64>(input) {
        Ok(KeyLiteral::sfixed64 { value: x })
    } else if let Ok(x) = str::parse::<u64>(input) {
        Ok(KeyLiteral::fixed64 { value: x })
    } else {
        Err(Error::InvalidNumberLiteral {
            core: ErrorCore::default(),
            as_str: input.to_string(),
        })
    }
}

pub fn number_literal(input: &str) -> ParseResult<KeyLiteral> {
    context(
        "number literal",
        map_res(recognize(tuple((opt(tag("-")), digit1))), number_to_typed),
    )(input)
}

pub fn query_exprs(input: &str) -> ParseResult<Vec<Query>> {
    context(
        "query expression",
        terminated(separated_list0(tag(","), map(query, |x| x)), opt(tag(","))),
    )(input)
}

pub fn query_filter(input: &str) -> ParseResult<QueryFilter> {
    context(
        "query filter",
        alt((
            map(tuple((ws0, number_literal)), |(_, x)| {
                QueryFilter::Equals(x)
            }),
            map(tuple((ws0, string_literal)), |(_, x)| {
                QueryFilter::Equals(x)
            }),
        )),
    )(input)
}

pub fn query(input: &str) -> ParseResult<Query> {
    context(
        "query",
        alt((
            map_res(
                tuple((
                    ws0,
                    identifier,
                    ws0,
                    tag("["),
                    cut(ws0),
                    cut(query_filter),
                    ws0,
                    tag("]"),
                    ws0,
                    tag("{"),
                    ws0,
                    cut(query_exprs),
                    ws0,
                    tag("}"),
                    ws0,
                )),
                |(_, ident, _, _, _, filter, _, _, __, _, _, exprs, _, _, _)| {
                    Query::from_filter_and_exprs(ident, filter, exprs)
                },
            ),
            map_res(
                tuple((
                    ws0,
                    identifier,
                    ws0,
                    tag("{"),
                    cut(ws0),
                    cut(query_exprs),
                    ws0,
                    tag("}"),
                    ws0,
                )),
                |(_, ident, _, _, _, exprs, _, _, _)| Query::from_exprs(ident, exprs),
            ),
            map_res(tuple((ws0, identifier, ws0)), |(_, ident, _)| {
                Query::new(ident)
            }),
        )),
    )(input)
}

///////////////////////////////////////////// parse_all ////////////////////////////////////////////

pub fn parse_all<T, F: Fn(&str) -> ParseResult<T> + Copy>(
    f: F,
) -> impl Fn(&str) -> Result<T, ParseError> {
    move |input| {
        let (rem, t) = match all_consuming(f)(input) {
            Ok((rem, t)) => (rem, t),
            Err(err) => match err {
                nom::Err::Incomplete(_) => {
                    panic!("all_consuming combinator should be all consuming");
                }
                nom::Err::Error(err) | nom::Err::Failure(err) => {
                    return Err(interpret_verbose_error(input, err));
                }
            },
        };
        if rem.is_empty() {
            Ok(t)
        } else {
            panic!("all_consuming combinator should be all consuming");
        }
    }
}

////////////////////////////////////////////// private /////////////////////////////////////////////

fn ws0(input: &str) -> ParseResult<()> {
    map(multispace0, |_| ())(input)
}

fn ws1(input: &str) -> ParseResult<()> {
    map(multispace1, |_| ())(input)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod test {
    use nom::combinator::{complete, cut};

    use super::*;

    fn parse_error(s: &'static str) -> ParseError {
        ParseError {
            string: s.to_string(),
        }
    }

    fn interpret_error_for_test<'a, T, F: FnMut(&'a str) -> ParseResult<T>>(
        mut f: F,
    ) -> impl FnMut(&'a str) -> Result<T, ParseError> {
        move |input| match f(input) {
            Ok((_, t)) => Ok(t),
            Err(err) => match err {
                nom::Err::Error(err) | nom::Err::Failure(err) => {
                    Err(interpret_verbose_error(input, err))
                }
                nom::Err::Incomplete(_) => {
                    panic!("incomplete should never happen in tests");
                }
            },
        }
    }

    #[test]
    fn identifier9() {
        assert_eq!(
            Identifier::must("__identifier9"),
            parse_all(identifier)("__identifier9").unwrap(),
        );
    }

    #[test]
    fn identifier_empty() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in identifier:

^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(identifier))))("").unwrap_err()
        );
    }

    #[test]
    fn identifier_dashes() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in identifier:
-not-identifier
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(identifier))))("-not-identifier")
                .unwrap_err()
        );
    }

    #[test]
    fn identifier_starts_with_number() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in identifier:
9identifier__
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(identifier))))("9identifier__")
                .unwrap_err()
        );
    }

    #[test]
    fn all_data_type() {
        assert_eq!(
            DataType::unit,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("unit").unwrap()
        );
        assert_eq!(
            DataType::int32,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("int32").unwrap()
        );
        assert_eq!(
            DataType::int64,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("int64").unwrap()
        );
        assert_eq!(
            DataType::uint32,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("uint32").unwrap()
        );
        assert_eq!(
            DataType::uint64,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("uint64").unwrap()
        );
        assert_eq!(
            DataType::sint32,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("sint32").unwrap()
        );
        assert_eq!(
            DataType::sint64,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("sint64").unwrap()
        );
        assert_eq!(
            DataType::fixed32,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("fixed32").unwrap()
        );
        assert_eq!(
            DataType::fixed64,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("fixed64").unwrap()
        );
        assert_eq!(
            DataType::sfixed32,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("sfixed32").unwrap()
        );
        assert_eq!(
            DataType::sfixed64,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("sfixed64").unwrap()
        );
        assert_eq!(
            DataType::float,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("float").unwrap()
        );
        assert_eq!(
            DataType::double,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("double").unwrap()
        );
        assert_eq!(
            DataType::Bool,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("bool").unwrap()
        );
        assert_eq!(
            DataType::bytes,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("bytes").unwrap()
        );
        assert_eq!(
            DataType::bytes16,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("bytes16").unwrap()
        );
        assert_eq!(
            DataType::bytes32,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("bytes32").unwrap()
        );
        assert_eq!(
            DataType::bytes64,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("bytes64").unwrap()
        );
        assert_eq!(
            DataType::string,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("string").unwrap()
        );
        assert_eq!(
            DataType::message,
            interpret_error_for_test(cut(complete(all_consuming(data_type))))("message").unwrap()
        );
    }

    #[test]
    fn bad_data_type() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in field number:
notatype
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_number))))("notatype")
                .unwrap_err()
        );
    }

    #[test]
    fn field_number1() {
        assert_eq!(
            FieldNumber::must(1),
            interpret_error_for_test(cut(complete(all_consuming(field_number))))("1").unwrap()
        );
    }

    #[test]
    fn field_number42() {
        assert_eq!(
            FieldNumber::must(42),
            interpret_error_for_test(cut(complete(all_consuming(field_number))))("42").unwrap()
        );
    }

    #[test]
    fn field_number_invalid() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in field number:
19000
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_number))))("19000")
                .unwrap_err()
        );
    }

    #[test]
    fn field_bytes32_identifier9_42() {
        assert_eq!(
            Field {
                ident: Identifier::must("__identifier9"),
                number: FieldNumber::must(42),
                ty: DataType::bytes32,
                breakout: false,
            },
            parse_all(field)("bytes32 __identifier9 = 42").unwrap(),
        );
    }

    #[test]
    fn field_breakout_bytes32_identifier9_42() {
        assert_eq!(
            Field {
                ident: Identifier::must("__identifier9"),
                number: FieldNumber::must(42),
                ty: DataType::bytes32,
                breakout: true,
            },
            interpret_error_for_test(cut(complete(all_consuming(field))))(
                "breakout bytes32 __identifier9 = 42"
            )
            .unwrap(),
        );
    }

    #[test]
    fn field_keyword_bytes32_identifier9_42() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in data type:
keyword bytes32 __identifier9 = 42
^

1: at line 1, in field type:
keyword bytes32 __identifier9 = 42
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(field))))(
                "keyword bytes32 __identifier9 = 42"
            )
            .unwrap_err(),
        );
    }

    #[test]
    fn object_identifer9_42() {
        assert_eq!(
            Object::new(
                Identifier::must("__identifier9"),
                FieldNumber::must(42),
                vec![]
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(object))))(
                "object __identifier9 = 42 {}"
            )
            .unwrap(),
        );
    }

    #[test]
    fn object_identifer9_42_whitespace() {
        assert_eq!(
            Object::new(
                Identifier::must("__identifier9"),
                FieldNumber::must(42),
                vec![]
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(object))))(
                "
                                                                           object
                                                                           __identifier9
                                                                           =
                                                                           42
                                                                           {
                                                                           }"
            )
            .unwrap(),
        );
    }

    #[test]
    fn object_identifer9_42_minimal_whitespace() {
        assert_eq!(
            Object::new(
                Identifier::must("__identifier9"),
                FieldNumber::must(42),
                vec![]
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(object))))(
                "object __identifier9=42{}"
            )
            .unwrap(),
        );
    }

    #[test]
    fn object_identifer9_42_with_field() {
        assert_eq!(
            Object::new(
                Identifier::must("__identifier9"),
                FieldNumber::must(42),
                vec![FieldDefinition::Field(Field {
                    ident: Identifier::must("field"),
                    number: FieldNumber::must(1),
                    ty: DataType::string,
                    breakout: false
                }),],
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(object))))(
                "object __identifier9 = 42 {
                    string field = 1;
                }"
            )
            .unwrap(),
        );
    }

    #[test]
    fn object_identifer9_42_missing_field_number() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in object type:
object __identifier9 {}
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(object))))(
                "object __identifier9 {}"
            )
            .unwrap_err(),
        );
    }

    #[test]
    fn map_string_identifer9_42() {
        assert_eq!(
            Map::new(
                Key::new(
                    Identifier::must("__identifier9"),
                    FieldNumber::must(42),
                    KeyDataType::string,
                    Direction::Forward,
                )
                .unwrap(),
                vec![]
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(map_field))))(
                "map string __identifier9 = 42 {}"
            )
            .unwrap(),
        );
    }

    #[test]
    fn map_missing_datatype_identifer9_42() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in key data type:
map __identifier9 = 42 {}
    ^

1: at line 1, in key type:
map __identifier9 = 42 {}
    ^

2: at line 1, in map type:
map __identifier9 = 42 {}
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(map_field))))(
                "map __identifier9 = 42 {}"
            )
            .unwrap_err(),
        );
    }

    #[test]
    fn map_string_missing_identifier_42() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in identifier:
map string = 42 {}
           ^

1: at line 1, in key type:
map string = 42 {}
    ^

2: at line 1, in map type:
map string = 42 {}
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(map_field))))("map string = 42 {}")
                .unwrap_err(),
        );
    }

    #[test]
    fn field_definition_field_bytes32_identifier9_42() {
        assert_eq!(
            FieldDefinition::Field(Field {
                ident: Identifier::must("__identifier9"),
                number: FieldNumber::must(42),
                ty: DataType::bytes32,
                breakout: false,
            }),
            parse_all(field_definition)("bytes32 __identifier9 = 42").unwrap(),
        );
    }

    #[test]
    fn field_definition_field_breakout_bytes32_identifier9_42() {
        assert_eq!(
            FieldDefinition::Field(Field {
                ident: Identifier::must("__identifier9"),
                number: FieldNumber::must(42),
                ty: DataType::bytes32,
                breakout: true,
            }),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "breakout bytes32 __identifier9 = 42"
            )
            .unwrap(),
        );
    }

    #[test]
    fn field_definition_field_keyword_bytes32_identifier9_42() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in join field:
keyword bytes32 __identifier9 = 42
^

1: at line 1, in field definition:
keyword bytes32 __identifier9 = 42
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "keyword bytes32 __identifier9 = 42"
            )
            .unwrap_err(),
        );
    }

    #[test]
    fn field_definition_object_identifer9_42() {
        assert_eq!(
            FieldDefinition::Object(
                Object::new(
                    Identifier::must("__identifier9"),
                    FieldNumber::must(42),
                    vec![]
                )
                .unwrap()
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "object __identifier9 = 42 {}"
            )
            .unwrap(),
        );
    }

    #[test]
    fn field_definition_object_identifer9_42_whitespace() {
        assert_eq!(
            FieldDefinition::Object(
                Object::new(
                    Identifier::must("__identifier9"),
                    FieldNumber::must(42),
                    vec![]
                )
                .unwrap()
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "
                                                                           object
                                                                           __identifier9
                                                                           =
                                                                           42
                                                                           {
                                                                           }"
            )
            .unwrap(),
        );
    }

    #[test]
    fn field_definition_object_identifer9_42_minimal_whitespace() {
        assert_eq!(
            FieldDefinition::Object(
                Object::new(
                    Identifier::must("__identifier9"),
                    FieldNumber::must(42),
                    vec![]
                )
                .unwrap()
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "object __identifier9=42{}"
            )
            .unwrap(),
        );
    }

    #[test]
    fn field_definition_object_identifer9_42_missing_field_number() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in object type:
object __identifier9 {}
^

1: at line 1, in field definition:
object __identifier9 {}
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "object __identifier9 {}"
            )
            .unwrap_err(),
        );
    }

    #[test]
    fn field_definition_map_string_identifer9_42() {
        assert_eq!(
            FieldDefinition::Map(
                Map::new(
                    Key::new(
                        Identifier::must("__identifier9"),
                        FieldNumber::must(42),
                        KeyDataType::string,
                        Direction::Forward,
                    )
                    .unwrap(),
                    vec![]
                )
                .unwrap()
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "map string __identifier9 = 42 {}"
            )
            .unwrap(),
        );
    }

    #[test]
    fn field_definition_map_missing_datatype_identifer9_42() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in key data type:
map __identifier9 = 42 {}
    ^

1: at line 1, in key type:
map __identifier9 = 42 {}
    ^

2: at line 1, in map type:
map __identifier9 = 42 {}
^

3: at line 1, in field definition:
map __identifier9 = 42 {}
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "map __identifier9 = 42 {}"
            )
            .unwrap_err(),
        );
    }

    #[test]
    fn field_definition_map_string_missing_identifier_42() {
        assert_eq!(
            parse_error(
                r#"0: at line 1, in identifier:
map string = 42 {}
           ^

1: at line 1, in key type:
map string = 42 {}
    ^

2: at line 1, in map type:
map string = 42 {}
^

3: at line 1, in field definition:
map string = 42 {}
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(field_definition))))(
                "map string = 42 {}"
            )
            .unwrap_err(),
        );
    }

    #[test]
    fn field_definition_join_key_success() {
        assert_eq!(
            FieldDefinition::Join(Join {
                ident: Identifier::must("spam"),
                number: FieldNumber::must(42),
                join_table: Identifier::must("Spam"),
                join_keys: vec![Identifier::must("a"), Identifier::must("b")],
            }),
            parse_all(field_definition)("join spam = 42 on Spam (a, b)").unwrap(),
        );
    }

    #[test]
    fn empty_table() {
        let key = vec![Key::new(
            Identifier::must("some_key"),
            FieldNumber::must(1),
            KeyDataType::string,
            Direction::Forward,
        )
        .unwrap()];
        assert_eq!(
            Table::new(
                Identifier::must("__identifier9"),
                FieldNumber::must(2),
                key,
                vec![]
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(table))))(
                "table __identifier9 (string some_key= 1) @ 2 {}"
            )
            .unwrap()
        );
    }

    #[test]
    fn table_error_no_number() {
        // We expect this table that lacks a table number to error out.
        const BAD_TABLE: &str = "table __identifier9 (string some_key= 1) {
            breakout string last_seen = 4;
        }";
        assert_eq!(
            parse_error(
                r#"0: at line 1, in table definition:
table __identifier9 (string some_key= 1) {
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(table))))(BAD_TABLE).unwrap_err()
        );
    }

    #[test]
    fn table_error_bad_field() {
        // TODO(rescrv): Make this have a better error message.
        const BAD_TABLE: &str = "table __identifier9 (string some_key= 1) @ 1 {
            breakout not_a_type last_seen = 4;
        }";
        assert_eq!(
            parse_error(
                r#"0: at line 2, in data type:
            breakout not_a_type last_seen = 4;
                     ^

1: at line 2, in field type:
            breakout not_a_type last_seen = 4;
            ^

2: at line 2, in field definition:
            breakout not_a_type last_seen = 4;
            ^

3: at line 2, in fields:
            breakout not_a_type last_seen = 4;
            ^

4: at line 2, in table body:
            breakout not_a_type last_seen = 4;
            ^

5: at line 1, in table definition:
table __identifier9 (string some_key= 1) @ 1 {
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(table))))(BAD_TABLE).unwrap_err()
        );
    }

    #[test]
    fn table_set_with_joins() {
        const TABLE_SET: &str = r#"
            table User (string user_id = 1) @ 1 {
                string email = 2;
                join avatar = 3 on Avatar (email);
                map string sessions = 4 {
                    join session = 1 on Session (user_id, sessions);
                };
            }

            table Avatar (string email = 1) @ 2 {
                string url = 2;
            }

            table Session (string user_id = 1, string session_id = 2) @ 3 {
                timestamp_micros first_valid = 3;
                timestamp_micros expires = 4;
            }
        "#;
        let user = Table::new(
            Identifier::must("User"),
            FieldNumber::must(1),
            vec![Key::new(
                Identifier::must("user_id"),
                FieldNumber::must(1),
                KeyDataType::string,
                Direction::Forward,
            )
            .unwrap()],
            vec![
                FieldDefinition::Field(
                    Field::new(
                        Identifier::must("email"),
                        FieldNumber::must(2),
                        DataType::string,
                        false,
                    )
                    .unwrap(),
                ),
                FieldDefinition::Join(
                    Join::new(
                        Identifier::must("avatar"),
                        FieldNumber::must(3),
                        Identifier::must("Avatar"),
                        vec![Identifier::must("email")],
                    )
                    .unwrap(),
                ),
                FieldDefinition::Map(
                    Map::new(
                        Key::new(
                            Identifier::must("sessions"),
                            FieldNumber::must(4),
                            KeyDataType::string,
                            Direction::Forward,
                        )
                        .unwrap(),
                        vec![FieldDefinition::Join(
                            Join::new(
                                Identifier::must("session"),
                                FieldNumber::must(1),
                                Identifier::must("Session"),
                                vec![Identifier::must("user_id"), Identifier::must("sessions")],
                            )
                            .unwrap(),
                        )],
                    )
                    .unwrap(),
                ),
            ],
        )
        .unwrap();
        let avatar = Table::new(
            Identifier::must("Avatar"),
            FieldNumber::must(2),
            vec![Key::new(
                Identifier::must("email"),
                FieldNumber::must(1),
                KeyDataType::string,
                Direction::Forward,
            )
            .unwrap()],
            vec![FieldDefinition::Field(
                Field::new(
                    Identifier::must("url"),
                    FieldNumber::must(2),
                    DataType::string,
                    false,
                )
                .unwrap(),
            )],
        )
        .unwrap();
        let session = Table::new(
            Identifier::must("Session"),
            FieldNumber::must(3),
            vec![
                Key::new(
                    Identifier::must("user_id"),
                    FieldNumber::must(1),
                    KeyDataType::string,
                    Direction::Forward,
                )
                .unwrap(),
                Key::new(
                    Identifier::must("session_id"),
                    FieldNumber::must(2),
                    KeyDataType::string,
                    Direction::Forward,
                )
                .unwrap(),
            ],
            vec![
                FieldDefinition::Field(
                    Field::new(
                        Identifier::must("first_valid"),
                        FieldNumber::must(3),
                        DataType::timestamp_micros,
                        false,
                    )
                    .unwrap(),
                ),
                FieldDefinition::Field(
                    Field::new(
                        Identifier::must("expires"),
                        FieldNumber::must(4),
                        DataType::timestamp_micros,
                        false,
                    )
                    .unwrap(),
                ),
            ],
        )
        .unwrap();
        assert_eq!(
            TableSet::new(vec![user, avatar, session,],).unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(table_set))))(TABLE_SET).unwrap()
        );
    }

    #[test]
    fn table_set_with_zero_key() {
        const TABLE_SET: &str = r#"
            table Global () @ 1 {
                bytes config = 1;
            }
        "#;
        let global = Table::new(
            Identifier::must("Global"),
            FieldNumber::must(1),
            vec![],
            vec![FieldDefinition::Field(
                Field::new(
                    Identifier::must("config"),
                    FieldNumber::must(1),
                    DataType::bytes,
                    false,
                )
                .unwrap(),
            )],
        )
        .unwrap();
        assert_eq!(
            TableSet::new(vec![global]).unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(table_set))))(TABLE_SET).unwrap()
        );
    }

    #[test]
    fn table_set_with_object() {
        const TABLE_SET: &str = r#"
            table ObjectTest () @ 1 {
                object obj = 1 {
                    uint64 x = 1;
                    uint64 y = 2;
                };
            }
        "#;
        let global = Table::new(
            Identifier::must("ObjectTest"),
            FieldNumber::must(1),
            vec![],
            vec![FieldDefinition::Object(
                Object::new(
                    Identifier::must("obj"),
                    FieldNumber::must(1),
                    vec![
                        FieldDefinition::Field(Field {
                            ident: Identifier::must("x"),
                            number: FieldNumber::must(1),
                            ty: DataType::uint64,
                            breakout: false,
                        }),
                        FieldDefinition::Field(Field {
                            ident: Identifier::must("y"),
                            number: FieldNumber::must(2),
                            ty: DataType::uint64,
                            breakout: false,
                        }),
                    ],
                )
                .unwrap(),
            )],
        )
        .unwrap();
        assert_eq!(
            TableSet::new(vec![global]).unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(table_set))))(TABLE_SET).unwrap()
        );
    }

    #[test]
    fn typical_query() {
        assert_eq!(
            Query::from_exprs(
                Identifier::must("User"),
                vec![
                    Query::new(Identifier::must("name")).unwrap(),
                    Query::new(Identifier::must("email")).unwrap(),
                    Query::from_exprs(
                        Identifier::must("password"),
                        vec![
                            Query::new(Identifier::must("algorithm")).unwrap(),
                            Query::new(Identifier::must("salt")).unwrap(),
                            Query::new(Identifier::must("hash")).unwrap(),
                        ]
                    )
                    .unwrap(),
                    Query::from_exprs(
                        Identifier::must("sessions"),
                        vec![Query::new(Identifier::must("expires_us")).unwrap(),]
                    )
                    .unwrap(),
                ]
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(query))))(
                r#"User {
                    name,
                    email,
                    password {
                        algorithm,
                        salt,
                        hash,
                    },
                    sessions {
                        expires_us,
                    },
                }"#
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_string_literal() {
        assert_eq!(
            KeyLiteral::string {
                value: "".to_string()
            },
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(r#""""#)
                .unwrap(),
        );
        assert_eq!(
            KeyLiteral::string {
                value: r#"""#.to_string()
            },
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(r#""\"""#)
                .unwrap(),
        );
        assert_eq!(
            KeyLiteral::string {
                value: r#"\"#.to_string()
            },
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(r#""\\""#)
                .unwrap(),
        );
        assert_eq!(
            KeyLiteral::string {
                value: r#""hello""world""#.to_string()
            },
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(
                r#""\"hello\"\"world\"""#
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_number_literal() {
        assert_eq!(
            KeyLiteral::sfixed32 { value: 0 },
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))("0").unwrap(),
        );
        assert_eq!(
            KeyLiteral::sfixed32 { value: i32::MIN },
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))("-2147483648")
                .unwrap(),
        );
        assert_eq!(
            KeyLiteral::sfixed32 { value: i32::MAX },
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))("2147483647")
                .unwrap(),
        );
        assert_eq!(
            KeyLiteral::fixed32 { value: u32::MAX },
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))("4294967295")
                .unwrap(),
        );
        assert_eq!(
            KeyLiteral::sfixed64 { value: i64::MIN },
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "-9223372036854775808"
            )
            .unwrap(),
        );
        assert_eq!(
            KeyLiteral::sfixed64 { value: i64::MAX },
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "9223372036854775807"
            )
            .unwrap(),
        );
        assert_eq!(
            KeyLiteral::fixed64 { value: u64::MAX },
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "18446744073709551615"
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_query_filter() {
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::string {
                value: "".to_string()
            }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))(r#""""#).unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::string {
                value: r#"""#.to_string()
            }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))(r#""\"""#)
                .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::string {
                value: r#"\"#.to_string()
            }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))(r#""\\""#)
                .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::string {
                value: r#""hello""world""#.to_string()
            }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))(
                r#""\"hello\"\"world\"""#
            )
            .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::sfixed32 { value: 0 }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))("0").unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::sfixed32 { value: i32::MIN }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))("-2147483648")
                .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::sfixed32 { value: i32::MAX }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))("2147483647")
                .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::fixed32 { value: u32::MAX }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))("4294967295")
                .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::sfixed64 { value: i64::MIN }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))(
                "-9223372036854775808"
            )
            .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::sfixed64 { value: i64::MAX }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))(
                "9223372036854775807"
            )
            .unwrap(),
        );
        assert_eq!(
            QueryFilter::Equals(KeyLiteral::fixed64 { value: u64::MAX }),
            interpret_error_for_test(cut(complete(all_consuming(query_filter))))(
                "18446744073709551615"
            )
            .unwrap(),
        );
    }

    #[test]
    fn typical_query_with_filter() {
        assert_eq!(
            Query::from_exprs(
                Identifier::must("User"),
                vec![
                    Query::new(Identifier::must("name")).unwrap(),
                    Query::new(Identifier::must("email")).unwrap(),
                    Query::from_exprs(
                        Identifier::must("password"),
                        vec![
                            Query::new(Identifier::must("algorithm")).unwrap(),
                            Query::new(Identifier::must("salt")).unwrap(),
                            Query::new(Identifier::must("hash")).unwrap(),
                        ]
                    )
                    .unwrap(),
                    Query::from_filter_and_exprs(
                        Identifier::must("sessions"),
                        QueryFilter::Equals(KeyLiteral::string {
                            value: "some-key".to_string()
                        }),
                        vec![Query::new(Identifier::must("expires_us")).unwrap(),]
                    )
                    .unwrap(),
                ]
            )
            .unwrap(),
            interpret_error_for_test(cut(complete(all_consuming(query))))(
                r#"User {
                    name,
                    email,
                    password {
                        algorithm,
                        salt,
                        hash,
                    },
                    sessions ["some-key"] {
                        expires_us,
                    },
                }"#
            )
            .unwrap(),
        );
    }

    #[test]
    fn query_error_bad_key() {
        assert_eq!(
            parse_error(
                r#"0: at line 9, in string literal:
                    sessions [some-key"] {
                              ^

1: at line 9, in query filter:
                    sessions [some-key"] {
                              ^

2: at line 8, in query:
                    },
                      ^

3: at line 2, in query expression:
                    name,
                    ^

4: at line 1, in query:
User {
^"#
            ),
            interpret_error_for_test(cut(complete(all_consuming(query))))(
                r#"User {
                    name,
                    email,
                    password {
                        algorithm,
                        salt,
                        hash,
                    },
                    sessions [some-key"] {
                        expires_us,
                    },
                }"#
            )
            .unwrap_err()
        );
    }
}
