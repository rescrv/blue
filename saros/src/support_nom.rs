use std::fmt::{Formatter, Write};

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_while},
    character::complete::{multispace0, multispace1, newline, none_of, one_of},
    combinator::{all_consuming, cut, map},
    error::{context, VerboseError, VerboseErrorKind},
    sequence::{delimited, pair},
    IResult, Offset,
};

////////////////////////////////////////// error handling //////////////////////////////////////////

pub type ParseResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

#[derive(Clone, Eq, PartialEq)]
pub struct ParseError {
    pub(crate) string: String,
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
                        "{index}: at line {line_number}: cannot parse input\n\
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

///////////////////////////////////// interpret_error_for_test /////////////////////////////////////

pub fn interpret_error_for_test<'a, T, F: FnMut(&'a str) -> ParseResult<T>>(
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

////////////////////////////////////////// string literal //////////////////////////////////////////

pub fn unescape(input: &str) -> String {
    let mut out: Vec<char> = Vec::new();
    let mut prev_was_escape = false;
    for c in input.chars() {
        if prev_was_escape && (c == '\"' || c == '\\') {
            out.push(c);
            prev_was_escape = false;
        } else if prev_was_escape && c == 'n' {
            out.push('\n');
            prev_was_escape = false;
        } else if c == '\\' {
            prev_was_escape = true;
        } else {
            out.push(c);
        }
    }
    out.into_iter().collect()
}

pub fn string_literal(input: &str) -> ParseResult<String> {
    context(
        "string literal",
        map(
            delimited(
                tag("\""),
                cut(alt((
                    escaped(none_of(r#"\""#), '\\', one_of(r#"\"n"#)),
                    tag(""),
                ))),
                tag("\""),
            ),
            unescape,
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

///////////////////////////////////////////// utilities ////////////////////////////////////////////

pub fn ws0(input: &str) -> ParseResult<()> {
    map(multispace0, |_| ())(input)
}

pub fn ws1(input: &str) -> ParseResult<()> {
    map(multispace1, |_| ())(input)
}

/// eat whitespace until newline
pub fn ewsunl(input: &str) -> ParseResult<()> {
    map(
        pair(
            take_while(|c: char| c.is_whitespace() && c != '\n'),
            newline,
        ),
        |_| (),
    )(input)
}
