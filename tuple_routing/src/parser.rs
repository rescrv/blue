use std::fmt::{Formatter, Write};

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{self as character, alpha1, alphanumeric1, multispace0},
    combinator::{all_consuming, cut, map, map_res, recognize},
    error::{context, VerboseError, VerboseErrorKind},
    multi::{many0, many0_count},
    sequence::{pair, tuple},
    IResult, Offset,
};

use prototk::FieldNumber;

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

//////////////////////////////////////////// TupleRouter ///////////////////////////////////////////

pub fn identifier(input: &str) -> ParseResult<String> {
    context(
        "identifier",
        map(
            recognize(pair(
                alt((alpha1, tag("_"))),
                many0_count(alt((alphanumeric1, tag("_")))),
            )),
            String::from,
        ),
    )(input)
}

pub fn field_number(input: &str) -> ParseResult<FieldNumber> {
    context("field number", map_res(character::u32, FieldNumber::new))(input)
}

pub fn schema(input: &str) -> ParseResult<tuple_key::Schema<()>> {
    context(
        "schema",
        map(
            many0(alt((
                map(
                    tuple((
                        ws0,
                        identifier,
                        ws0,
                        tag("="),
                        ws0,
                        field_number,
                        ws0,
                        tag("{"),
                        cut(ws0),
                        schema,
                        cut(ws0),
                        tag("}"),
                    )),
                    |(_, ident, _, _, _, number, _, _, _, schema, _, _)| ((number, ident), schema),
                ),
                map(
                    tuple((ws0, identifier, ws0, tag("="), ws0, field_number, tag(";"))),
                    |(_, ident, _, _, _, number, _)| {
                        ((number, ident), tuple_key::Schema::new((), [].into_iter()))
                    },
                ),
            ))),
            |children| {
                // TODO(rescrv):  How to handle duplicate routes?
                tuple_key::Schema::new((), children.into_iter())
            },
        ),
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
}
