use std::fmt::{Formatter, Write};

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag},
    character::complete::{digit1, multispace0, multispace1, none_of, one_of},
    combinator::{all_consuming, map, map_res, opt, recognize},
    error::{context, VerboseError, VerboseErrorKind},
    multi::{separated_list0, separated_list1},
    number::complete::double,
    sequence::{delimited, terminated, tuple},
    IResult, Offset,
};

use zerror_core::ErrorCore;

use crate::{Error, Query};

////////////////////////////////////////// error handling //////////////////////////////////////////

type ParseResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

#[derive(Clone, Eq, PartialEq)]
pub struct ParseError {
    string: String,
}

impl ParseError {
    pub fn what(&self) -> &str {
        &self.string
    }
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

fn interpret_verbose_error(input: &'_ str, err: VerboseError<&'_ str>) -> ParseError {
    let mut result = String::new();
    let mut index = 0;
    let mut seen_eof = false;
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
            VerboseErrorKind::Nom(nom::error::ErrorKind::Eof) => {
                if !seen_eof {
                    write!(
                        &mut result,
                        "{index}: at line {line_number}: end of file\n\
                   {line}\n\
                   {caret:>column$}\n\n",
                        index = index,
                        line_number = line_number,
                        line = line,
                        caret = '^',
                        column = column_number,
                    )
                    .unwrap();
                }
                index += 1;
                seen_eof = true;
            }
            VerboseErrorKind::Nom(_) => {}
        };
    }
    ParseError {
        string: result.trim().to_string(),
    }
}

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

/////////////////////////////////////////// bool literal ///////////////////////////////////////////

fn ternary_literal(input: &str) -> ParseResult<Query> {
    context(
        "ternary literal",
        alt((
            map(tag("null"), |_| Query::Null),
            map(tag("true"), |_| Query::True),
            map(tag("false"), |_| Query::False),
        )),
    )(input)
}

////////////////////////////////////////// number literal //////////////////////////////////////////

fn number_to_typed(input: &str) -> Result<Query, Error> {
    if let Ok(x) = str::parse::<i64>(input) {
        Ok(Query::I64(x))
    } else if let Ok(x) = str::parse::<u64>(input) {
        Ok(Query::U64(x))
    } else if let Ok(x) = str::parse::<f64>(input) {
        Ok(Query::F64(x))
    } else {
        Err(Error::InvalidNumberLiteral {
            core: ErrorCore::default(),
            as_str: input.to_string(),
        })
    }
}

fn number_literal(input: &str) -> ParseResult<Query> {
    context(
        "number literal",
        alt((
            map_res(recognize(double), number_to_typed),
            map_res(recognize(tuple((opt(tag("-")), digit1))), number_to_typed),
        )),
    )(input)
}

////////////////////////////////////////// string literal //////////////////////////////////////////

fn unescape(input: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    let mut prev_was_escape = false;
    // TODO(rescrv):  Look into matching JSON exactly.
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

fn string_literal(input: &str) -> ParseResult<String> {
    context(
        "string literal",
        map(
            delimited(
                tag("\""),
                alt((escaped(none_of(r#"\""#), '\\', one_of(r#"\""#)), tag(""))),
                tag("\""),
            ),
            |x: &str| unescape(x),
        ),
    )(input)
}

fn string_query(input: &str) -> ParseResult<Query> {
    context(
        "string literal",
        map(
            delimited(
                tag("\""),
                alt((escaped(none_of(r#"\""#), '\\', one_of(r#"\""#)), tag(""))),
                tag("\""),
            ),
            |x: &str| Query::String(unescape(x)),
        ),
    )(input)
}

/////////////////////////////////////////////// array //////////////////////////////////////////////

fn array_query(input: &str) -> ParseResult<Query> {
    context(
        "array query",
        map(
            tuple((
                tag("["),
                ws0,
                terminated(
                    separated_list0(tuple((ws0, tag(","), ws0)), query),
                    opt(tag(",")),
                ),
                ws0,
                tag("]"),
            )),
            |(_, _, values, _, _)| Query::Array(values),
        ),
    )(input)
}

////////////////////////////////////////////// object //////////////////////////////////////////////

fn object_query(input: &str) -> ParseResult<Query> {
    context(
        "object query",
        map(
            tuple((
                tag("{"),
                ws0,
                terminated(
                    separated_list0(
                        tuple((ws0, tag(","), ws0)),
                        alt((
                            map(
                                tuple((string_literal, ws0, tag(":"), ws0, query)),
                                |(string, _, _, _, query)| (string, query),
                            ),
                            map(string_literal, |string| (string, Query::Any)),
                        )),
                    ),
                    opt(tag(",")),
                ),
                ws0,
                tag("}"),
            )),
            |(_, _, queries, _, _)| Query::Object(queries),
        ),
    )(input)
}

///////////////////////////////////////////// or_query /////////////////////////////////////////////

fn or_query(input: &str) -> ParseResult<Query> {
    context(
        "or query",
        map(
            separated_list1(tuple((ws1, tag("or"), ws1)), query_one),
            |mut queries| {
                if queries.is_empty() {
                    Query::Or(vec![])
                } else if queries.len() == 1 {
                    queries.pop().unwrap()
                } else {
                    Query::Or(queries)
                }
            },
        ),
    )(input)
}

///////////////////////////////////////////// query_one ////////////////////////////////////////////

pub fn query_one(input: &str) -> ParseResult<Query> {
    alt((
        ternary_literal,
        number_literal,
        string_query,
        array_query,
        object_query,
    ))(input)
}

/////////////////////////////////////////////// query //////////////////////////////////////////////

pub fn query(input: &str) -> ParseResult<Query> {
    context("query", alt((or_query, query_one)))(input)
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
    fn null() {
        assert_eq!(Query::Null, parse_all(ternary_literal)("null").unwrap(),);
    }

    #[test]
    fn bool_true() {
        assert_eq!(Query::True, parse_all(ternary_literal)("true").unwrap(),);
    }

    #[test]
    fn bool_false() {
        assert_eq!(Query::False, parse_all(ternary_literal)("false").unwrap(),);
    }

    #[test]
    fn parse_number_literal() {
        assert_eq!(
            Query::I64(0),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))("0").unwrap(),
        );
        assert_eq!(
            Query::I64(i64::MIN),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "-9223372036854775808"
            )
            .unwrap(),
        );
        assert_eq!(
            Query::I64(i64::MAX),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "9223372036854775807"
            )
            .unwrap(),
        );
        assert_eq!(
            Query::U64(u64::MAX),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "18446744073709551615"
            )
            .unwrap(),
        );
        assert_eq!(
            Query::F64(std::f64::consts::PI),
            interpret_error_for_test(cut(complete(all_consuming(number_literal))))(
                "3.14159265358979323846264338327950288"
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_string_query() {
        assert_eq!(
            Query::String("".to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_query))))(r#""""#).unwrap(),
        );
        assert_eq!(
            Query::String(r#"""#.to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_query))))(r#""\"""#)
                .unwrap(),
        );
        assert_eq!(
            Query::String(r#"\"#.to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_query))))(r#""\\""#)
                .unwrap(),
        );
        assert_eq!(
            Query::String(r#""hello""world""#.to_string()),
            interpret_error_for_test(cut(complete(all_consuming(string_query))))(
                r#""\"hello\"\"world\"""#
            )
            .unwrap(),
        );
    }

    #[test]
    fn parse_array_query() {
        assert_eq!(
            Query::Array(vec![]),
            interpret_error_for_test(cut(complete(all_consuming(array_query))))("[]").unwrap()
        );
        assert_eq!(
            Query::Array(vec![
                Query::True,
                Query::I64(i64::MIN),
                Query::F64(std::f64::consts::PI),
                Query::String("hello world".to_string()),
                Query::Array(vec![
                    Query::String("hello".to_string()),
                    Query::String("world".to_string()),
                ]),
            ]),
            interpret_error_for_test(cut(complete(all_consuming(array_query))))(
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
    fn parse_object_query() {
        assert_eq!(
            Query::Object(vec![]),
            interpret_error_for_test(cut(complete(all_consuming(object_query))))("{}").unwrap()
        );
        assert_eq!(
            Query::Object(vec![
                ("bool".to_string(), Query::True),
                ("pi".to_string(), Query::F64(std::f64::consts::PI)),
                (
                    "hello_world".to_string(),
                    Query::Array(vec![
                        Query::String("hello".to_string()),
                        Query::String("world".to_string()),
                    ])
                ),
            ]),
            interpret_error_for_test(cut(complete(all_consuming(object_query))))(
                r#"{
                    "bool": true,
                    "pi": 3.14159265358979323846264338327950288,
                    "hello_world": ["hello", "world"],
                }"#
            )
            .unwrap()
        );
    }
}
