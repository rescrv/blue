use std::fmt::{Formatter, Write};

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag},
    character::complete::{alpha1, alphanumeric1, digit1, multispace0, none_of, one_of},
    combinator::{all_consuming, cut, map, map_res, opt, recognize},
    error::{context, VerboseError, VerboseErrorKind},
    multi::{many0_count, separated_list0},
    number::complete::double,
    sequence::{delimited, pair, terminated, tuple},
    IResult, Offset,
};

use crate::{Error, Value};

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

//////////////////////////////////////////// identifier ////////////////////////////////////////////

pub fn identifier(input: &str) -> ParseResult<String> {
    context(
        "identifier",
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                many0_count(alt((alphanumeric1, tag("_")))),
            )),
            |ident: &str| ident.to_string(),
        ),
    )(input)
}

/////////////////////////////////////////// bool literal ///////////////////////////////////////////

pub fn bool_literal(input: &str) -> ParseResult<Value> {
    context(
        "bool literal",
        alt((
            map(tag("true"), |_| Value::Bool(true)),
            map(tag("false"), |_| Value::Bool(false)),
        )),
    )(input)
}

////////////////////////////////////////// number literal //////////////////////////////////////////

fn number_to_typed(input: &str) -> Result<Value, Error> {
    if let Ok(x) = str::parse::<i64>(input) {
        Ok(Value::I64(x))
    } else if let Ok(x) = str::parse::<u64>(input) {
        Ok(Value::U64(x))
    } else if let Ok(x) = str::parse::<f64>(input) {
        Ok(Value::F64(x))
    } else {
        Err(Error::InvalidNumberLiteral {
            as_str: input.to_string(),
        })
    }
}

pub fn number_literal(input: &str) -> ParseResult<Value> {
    context(
        "number literal",
        alt((
            map_res(recognize(double), number_to_typed),
            map_res(recognize(tuple((opt(tag("-")), digit1))), number_to_typed),
        )),
    )(input)
}

////////////////////////////////////////// string literal //////////////////////////////////////////

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

pub fn string_literal(input: &str) -> ParseResult<Value> {
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
            |x: &str| Value::String(unescape(x)),
        ),
    )(input)
}

/////////////////////////////////////////////// array //////////////////////////////////////////////

pub fn array_literal(input: &str) -> ParseResult<Value> {
    context(
        "array literal",
        map(
            tuple((
                tag("["),
                ws0,
                cut(terminated(
                    separated_list0(
                        tag(","),
                        map(tuple((ws0, value, ws0)), |(_, value, _)| value),
                    ),
                    opt(tag(",")),
                )),
                ws0,
                tag("]"),
            )),
            |(_, _, values, _, _)| Value::Array(values.into()),
        ),
    )(input)
}

/////////////////////////////////////////////// array //////////////////////////////////////////////

pub fn object_literal(input: &str) -> ParseResult<Value> {
    context(
        "object literal",
        map(
            tuple((
                tag("{"),
                ws0,
                cut(terminated(
                    separated_list0(
                        tag(","),
                        map(
                            tuple((ws0, identifier, ws0, tag(":"), ws0, value, ws0)),
                            |(_, ident, _, _, _, value, _)| (ident, value),
                        ),
                    ),
                    opt(tag(",")),
                )),
                ws0,
                tag("}"),
            )),
            |(_, _, values, _, _)| Value::Object(values.into_iter().collect()),
        ),
    )(input)
}

/////////////////////////////////////////////// value //////////////////////////////////////////////

pub fn value(input: &str) -> ParseResult<Value> {
    context(
        "value",
        alt((
            bool_literal,
            number_literal,
            string_literal,
            array_literal,
            object_literal,
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

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod test {
    use nom::combinator::{complete, cut};

    use crate::Map;

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
            "__identifier9".to_string(),
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
    fn bool_true() {
        assert_eq!(Value::Bool(true), parse_all(bool_literal)("true").unwrap(),);
    }

    #[test]
    fn bool_false() {
        assert_eq!(
            Value::Bool(false),
            parse_all(bool_literal)("false").unwrap(),
        );
    }

    #[test]
    fn parse_number_literal() {
        assert_eq!(
            Value::I64(0),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))("0").unwrap(),
        );
        assert_eq!(
            Value::I64(i64::MIN),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "-9223372036854775808"
            )
            .unwrap(),
        );
        assert_eq!(
            Value::I64(i64::MAX),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "9223372036854775807"
            )
            .unwrap(),
        );
        assert_eq!(
            Value::U64(u64::MAX),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "18446744073709551615"
            )
            .unwrap(),
        );
        assert_eq!(
            Value::F64(std::f64::consts::PI),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "3.14159265358979323846264338327950288"
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_string_literal() {
        assert_eq!(
            Value::String("".to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(r#""""#)
                .unwrap(),
        );
        assert_eq!(
            Value::String(r#"""#.to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(r#""\"""#)
                .unwrap(),
        );
        assert_eq!(
            Value::String(r#"\"#.to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(r#""\\""#)
                .unwrap(),
        );
        assert_eq!(
            Value::String(r#""hello""world""#.to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_literal))))(
                r#""\"hello\"\"world\"""#
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_array_literal() {
        assert_eq!(
            Value::Array(vec![].into()),
            interpret_error_for_test(cut(complete(all_consuming(array_literal))))("[]").unwrap()
        );
        assert_eq!(
            Value::Array(vec![
                Value::Bool(true),
                Value::I64(i64::MIN),
                Value::F64(std::f64::consts::PI),
                Value::String("hello world".to_string()),
                Value::Array(vec![
                    Value::String("hello".to_string()),
                    Value::String("world".to_string()),
                ].into()),
            ].into()),
            interpret_error_for_test(cut(complete(all_consuming(array_literal))))(
                r#"[
                    true,
                    -9223372036854775808,
                    3.14159265358979323846264338327950288,
                    "hello world",
                    ["hello", "world"],
                ]"#
            )
            .unwrap()
        );
    }

    #[test]
    fn parse_object_literal() {
        assert_eq!(
            Value::Object(Map::default()),
            interpret_error_for_test(cut(complete(all_consuming(object_literal))))("{}").unwrap()
        );
        assert_eq!(
            Value::Object(
                vec![
                    ("bool".to_string(), Value::Bool(true)),
                    ("pi".to_string(), Value::F64(std::f64::consts::PI)),
                    (
                        "hello_world".to_string(),
                        Value::Array(vec![
                            Value::String("hello".to_string()),
                            Value::String("world".to_string()),
                        ].into())
                    ),
                ]
                .into_iter()
                .collect()
            ),
            interpret_error_for_test(cut(complete(all_consuming(object_literal))))(
                r#"{
                    bool: true,
                    pi: 3.14159265358979323846264338327950288,
                    hello_world: ["hello", "world"],
                }"#
            )
            .unwrap()
        );
    }
}
