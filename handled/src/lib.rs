#![doc = include_str!("../README.md")]

use std::fmt::Debug;

/// A symbolic expression: the fundamental data structure for representing structured data.
///
/// S-expressions provide a uniform representation for both code and data, enabling
/// homoiconic transformations where programs can manipulate other programs as data.
///
/// # Examples
///
/// ```
/// use handled::SExpr;
///
/// // An atom representing a symbol
/// let symbol = SExpr::Atom("hello".to_string());
///
/// // A list representing a function call
/// let call = SExpr::List(vec![
///     SExpr::Atom("add".to_string()),
///     SExpr::Atom("1".to_string()),
///     SExpr::Atom("2".to_string()),
/// ]);
/// assert_eq!(call.to_string(), "(add 1 2)");
/// ```
#[derive(Debug, PartialEq, Clone)]
pub enum SExpr {
    /// An atomic value: a symbol, number, string, or other indivisible token.
    Atom(String),
    /// A list of S-expressions, where the first element conventionally names the form.
    List(Vec<SExpr>),
}

impl std::fmt::Display for SExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SExpr::Atom(s) => write!(f, "{}", s),
            SExpr::List(l) => {
                write!(f, "(")?;
                for (i, expr) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", expr)?;
                }
                write!(f, ")")
            }
        }
    }
}

/// A recursive descent parser for S-expressions.
///
/// Transforms a string representation into an [`SExpr`] abstract syntax tree.
/// The parser handles atoms, quoted strings with escape sequences, nested lists,
/// and the quote shorthand (`'expr` → `(quote expr)`).
///
/// # Examples
///
/// ```
/// use handled::parse;
///
/// let expr = parse("(doc (h1 \"Hello\"))").unwrap();
/// assert_eq!(expr.to_string(), "(doc (h1 \"Hello\"))");
/// ```
pub fn parse(input: &str) -> SResult<SExpr> {
    Parser::new(input).parse()
}

struct Parser<'a> {
    input: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    position: usize,
}

impl<'a> Parser<'a> {
    /// Creates a new parser for the given input string.
    pub fn new(input: &'a str) -> Self {
        Parser {
            input,
            chars: input.char_indices().peekable(),
            position: 0,
        }
    }

    /// Parses the input and returns the resulting S-expression.
    ///
    /// # Errors
    ///
    /// Returns an error if the input contains:
    /// - Unclosed lists (missing `)`)
    /// - Unclosed quoted strings (missing `"`)
    /// - Unexpected end of input
    pub fn parse(&mut self) -> SResult<SExpr> {
        self.consume_whitespace();
        match self.peek_char() {
            Some('\'') => self.parse_quoted(),
            Some('(') => self.parse_list(),
            Some('"') => self.parse_quoted_string(),
            Some(_) => self.parse_atom(),
            None => Err(SError::new("parse")
                .with_code("unexpected-eof")
                .with_message("Unexpected end of input")
                .with_atom_field("position", self.position)),
        }
    }

    fn parse_quoted(&mut self) -> SResult<SExpr> {
        self.next_char(); // consume the '
        let expr = self.parse()?;
        Ok(SExpr::List(vec![SExpr::Atom("quote".to_string()), expr]))
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|&(_, ch)| ch)
    }

    fn next_char(&mut self) -> Option<char> {
        if let Some((idx, ch)) = self.chars.next() {
            self.position = idx + ch.len_utf8();
            Some(ch)
        } else {
            None
        }
    }

    fn consume_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn parse_list(&mut self) -> SResult<SExpr> {
        let start_position = self.position;
        self.next_char(); // consume '('
        let mut list = Vec::new();
        loop {
            self.consume_whitespace();
            match self.peek_char() {
                Some(')') => {
                    self.next_char();
                    return Ok(SExpr::List(list));
                }
                Some(_) => {
                    list.push(self.parse()?);
                }
                None => {
                    return Err(SError::new("parse")
                        .with_code("unclosed-list")
                        .with_message("Unclosed list: expected ')' but reached end of input")
                        .with_atom_field("start_position", start_position)
                        .with_atom_field("current_position", self.position)
                        .with_atom_field("list_elements_parsed", list.len()));
                }
            }
        }
    }

    fn parse_quoted_string(&mut self) -> SResult<SExpr> {
        let start_position = self.position;
        self.next_char(); // consume opening '"'
        let mut result = String::from("\"");

        loop {
            match self.peek_char() {
                None => {
                    return Err(SError::new("parse")
                        .with_code("unclosed-string")
                        .with_message(
                            "Unclosed quoted string: expected '\"' but reached end of input",
                        )
                        .with_atom_field("start_position", start_position)
                        .with_atom_field("current_position", self.position));
                }
                Some('\\') => {
                    self.next_char(); // consume '\'
                    match self.peek_char() {
                        Some(ch) => {
                            result.push('\\');
                            result.push(ch);
                            self.next_char();
                        }
                        None => {
                            return Err(SError::new("parse")
                                .with_code("unclosed-string")
                                .with_message("Unclosed quoted string: escape at end of input")
                                .with_atom_field("start_position", start_position)
                                .with_atom_field("current_position", self.position));
                        }
                    }
                }
                Some('"') => {
                    result.push('"');
                    self.next_char(); // consume closing '"'
                    return Ok(SExpr::Atom(result));
                }
                Some(ch) => {
                    result.push(ch);
                    self.next_char();
                }
            }
        }
    }

    fn parse_atom(&mut self) -> SResult<SExpr> {
        let start = self.position;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_whitespace() || ch == '(' || ch == ')' {
                break;
            }
            self.next_char();
        }
        let end = self.position;
        Ok(SExpr::Atom(self.input[start..end].to_string()))
    }
}

////////////////////////////////////////////// SError //////////////////////////////////////////////

pub type SResult<T> = Result<T, SError>;

#[derive(Debug, Clone, PartialEq)]
pub struct SError {
    detail: SExpr,
}

impl SError {
    /// Builds a new error anchored to a specific phase of processing.
    pub fn new(phase: &str) -> Self {
        SError {
            detail: SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                field("phase", atom(phase)),
            ]),
        }
    }

    /// Annotates the error with a machine-readable code.
    pub fn with_code(self, code: &str) -> Self {
        self.with_field("code", atom(code))
    }

    /// Adds a human-readable message to the error.
    pub fn with_message(self, message: &str) -> Self {
        self.with_field("message", string_literal(message))
    }

    /// Adds an arbitrary field with an atomic value.
    pub fn with_atom_field<T: ToString>(self, name: &str, value: T) -> Self {
        self.with_field(name, atom(value))
    }

    /// Adds an arbitrary field with a string literal value.
    pub fn with_string_field(self, name: &str, value: &str) -> Self {
        self.with_field(name, string_literal(value))
    }

    /// Adds an arbitrary field expressed as an `SExpr`.
    pub fn with_field(mut self, name: &str, value: SExpr) -> Self {
        if let SExpr::List(entries) = &mut self.detail {
            entries.push(field(name, value));
        }
        self
    }

    /// Returns the underlying detail `SExpr`.
    pub fn detail(&self) -> &SExpr {
        &self.detail
    }

    /// Consumes the error and returns the inner `SExpr` detail.
    pub fn into_detail(self) -> SExpr {
        self.detail
    }
}

impl std::fmt::Display for SError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

impl std::error::Error for SError {}

impl From<SExpr> for SError {
    fn from(detail: SExpr) -> Self {
        SError { detail }
    }
}

fn field(name: &str, value: SExpr) -> SExpr {
    SExpr::List(vec![SExpr::Atom(name.to_string()), value])
}

fn atom<T: ToString>(value: T) -> SExpr {
    SExpr::Atom(value.to_string())
}

fn string_literal(value: &str) -> SExpr {
    SExpr::Atom(format!("\"{}\"", escape_string(value)))
}

/// Escapes special characters in a string for S-expression representation.
///
/// Handles backslash, double-quote, newline, carriage return, and tab.
pub fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Creates a quoted string atom with proper escaping.
///
/// Wraps the escaped string in double quotes to create a valid S-expression string atom.
pub fn string_atom(s: &str) -> SExpr {
    SExpr::Atom(format!("\"{}\"", escape_string(s)))
}

/// Extracts string content from an atom, handling quoted strings.
///
/// If the atom is a quoted string (starts and ends with `"`), unescapes and returns
/// the inner content. Otherwise returns the atom value as-is.
/// Returns an empty string for non-atom expressions.
pub fn extract_string(expr: &SExpr) -> String {
    match expr {
        SExpr::Atom(s) => {
            if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                unescape_string(&s[1..s.len() - 1])
            } else {
                s.clone()
            }
        }
        _ => String::new(),
    }
}

/// Processes escape sequences in a string, converting them to their literal characters.
///
/// Recognizes standard escape sequences: `\n`, `\r`, `\t`, `\\`, `\"`.
/// Unknown escape sequences are preserved literally.
pub fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(next);
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_atom() {
        let mut parser = Parser::new("foo");
        assert_eq!(parser.parse(), Ok(SExpr::Atom("foo".to_string())));
    }

    #[test]
    fn test_simple_list() {
        let mut parser = Parser::new("(foo bar)");
        assert_eq!(
            parser.parse(),
            Ok(SExpr::List(vec![
                SExpr::Atom("foo".to_string()),
                SExpr::Atom("bar".to_string())
            ]))
        );
    }

    #[test]
    fn test_nested_list() {
        let mut parser = Parser::new("(foo (bar baz) qux)");
        assert_eq!(
            parser.parse(),
            Ok(SExpr::List(vec![
                SExpr::Atom("foo".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("bar".to_string()),
                    SExpr::Atom("baz".to_string())
                ]),
                SExpr::Atom("qux".to_string())
            ]))
        );
    }

    #[test]
    fn test_display_atom() {
        let expr = SExpr::Atom("foo".to_string());
        assert_eq!(expr.to_string(), "foo");
    }

    #[test]
    fn test_display_list() {
        let expr = SExpr::List(vec![
            SExpr::Atom("foo".to_string()),
            SExpr::Atom("bar".to_string()),
        ]);
        assert_eq!(expr.to_string(), "(foo bar)");
    }

    #[test]
    fn test_round_trip() {
        let input = "(foo (bar baz) qux)";
        let mut parser = Parser::new(input);
        let ast = parser.parse().unwrap();
        assert_eq!(ast.to_string(), input);
    }

    #[test]
    fn test_empty_list() {
        let mut parser = Parser::new("()");
        assert_eq!(parser.parse(), Ok(SExpr::List(vec![])));
    }

    #[test]
    fn test_list_with_whitespace() {
        let mut parser = Parser::new(" ( foo   bar ) ");
        let ast = parser.parse().unwrap();
        assert_eq!(
            ast,
            SExpr::List(vec![
                SExpr::Atom("foo".to_string()),
                SExpr::Atom("bar".to_string())
            ])
        );
        assert_eq!(ast.to_string(), "(foo bar)");
    }

    #[test]
    fn unclosed_list_error() {
        let mut parser = Parser::new("(foo bar");
        let err = parser.parse().unwrap_err();
        assert!(err.to_string().contains("unclosed-list"));
    }

    #[test]
    fn quoted_string_simple() {
        let mut parser = Parser::new(r#""hello""#);
        assert_eq!(parser.parse(), Ok(SExpr::Atom(r#""hello""#.to_string())));
    }

    #[test]
    fn quoted_string_with_spaces() {
        let mut parser = Parser::new(r#""hello world""#);
        assert_eq!(
            parser.parse(),
            Ok(SExpr::Atom(r#""hello world""#.to_string()))
        );
    }

    #[test]
    fn quoted_string_with_escapes() {
        let mut parser = Parser::new(r#""hello \"world\"""#);
        assert_eq!(
            parser.parse(),
            Ok(SExpr::Atom(r#""hello \"world\"""#.to_string()))
        );
    }

    #[test]
    fn list_with_quoted_strings() {
        let mut parser = Parser::new(r#"(obj ("key" "value with spaces"))"#);
        assert_eq!(
            parser.parse(),
            Ok(SExpr::List(vec![
                SExpr::Atom("obj".to_string()),
                SExpr::List(vec![
                    SExpr::Atom(r#""key""#.to_string()),
                    SExpr::Atom(r#""value with spaces""#.to_string())
                ])
            ]))
        );
    }

    #[test]
    fn unclosed_quoted_string_error() {
        let mut parser = Parser::new(r#""hello"#);
        let err = parser.parse().unwrap_err();
        assert!(err.to_string().contains("unclosed-string"));
    }

    #[test]
    fn empty_input_error() {
        let mut parser = Parser::new("");
        let err = parser.parse().unwrap_err();
        assert!(err.to_string().contains("unexpected-eof"));
    }

    #[test]
    fn whitespace_only_input_error() {
        let mut parser = Parser::new("   \n\t  ");
        let err = parser.parse().unwrap_err();
        assert!(err.to_string().contains("unexpected-eof"));
    }

    #[test]
    fn quoted_syntax_shorthand() {
        let mut parser = Parser::new("'foo");
        assert_eq!(parser.parse().unwrap().to_string(), "(quote foo)");
    }

    #[test]
    fn quoted_list_shorthand() {
        let mut parser = Parser::new("'(a b c)");
        assert_eq!(parser.parse().unwrap().to_string(), "(quote (a b c))");
    }

    #[test]
    fn deeply_nested_lists() {
        let mut parser = Parser::new("(a (b (c (d (e (f))))))");
        assert_eq!(
            parser.parse().unwrap().to_string(),
            "(a (b (c (d (e (f))))))"
        );
    }

    #[test]
    fn atom_with_special_characters() {
        let mut parser = Parser::new("foo-bar_baz123");
        assert_eq!(
            parser.parse().unwrap(),
            SExpr::Atom("foo-bar_baz123".to_string())
        );
    }

    #[test]
    fn atom_with_symbols() {
        let mut parser = Parser::new("+-*/<>=!?");
        assert_eq!(
            parser.parse().unwrap(),
            SExpr::Atom("+-*/<>=!?".to_string())
        );
    }

    #[test]
    fn numeric_atom_positive() {
        let mut parser = Parser::new("123");
        assert_eq!(parser.parse().unwrap(), SExpr::Atom("123".to_string()));
    }

    #[test]
    fn numeric_atom_negative() {
        let mut parser = Parser::new("-456");
        assert_eq!(parser.parse().unwrap(), SExpr::Atom("-456".to_string()));
    }

    #[test]
    fn numeric_atom_float() {
        let mut parser = Parser::new("3.14159");
        assert_eq!(parser.parse().unwrap(), SExpr::Atom("3.14159".to_string()));
    }

    #[test]
    fn quoted_string_with_newlines() {
        let mut parser = Parser::new(r#""hello\nworld""#);
        assert_eq!(
            parser.parse().unwrap(),
            SExpr::Atom(r#""hello\nworld""#.to_string())
        );
    }

    #[test]
    fn quoted_string_with_tabs() {
        let mut parser = Parser::new(r#""col1\tcol2""#);
        assert_eq!(
            parser.parse().unwrap(),
            SExpr::Atom(r#""col1\tcol2""#.to_string())
        );
    }

    #[test]
    fn quoted_string_with_backslashes() {
        let mut parser = Parser::new(r#""path\\to\\file""#);
        assert_eq!(
            parser.parse().unwrap(),
            SExpr::Atom(r#""path\\to\\file""#.to_string())
        );
    }

    #[test]
    fn escape_at_end_of_string_error() {
        let mut parser = Parser::new(r#""hello\"#);
        let err = parser.parse().unwrap_err();
        assert!(err.to_string().contains("unclosed-string"));
    }

    #[test]
    fn nested_lists_with_multiple_levels() {
        let input = "(doc (h1 \"Title\") (ul (li \"A\") (li \"B\")) (p \"End\"))";
        let mut parser = Parser::new(input);
        assert_eq!(parser.parse().unwrap().to_string(), input);
    }

    #[test]
    fn list_with_mixed_content() {
        let mut parser = Parser::new("(func 123 \"str\" symbol (nested))");
        let expected = SExpr::List(vec![
            SExpr::Atom("func".to_string()),
            SExpr::Atom("123".to_string()),
            SExpr::Atom("\"str\"".to_string()),
            SExpr::Atom("symbol".to_string()),
            SExpr::List(vec![SExpr::Atom("nested".to_string())]),
        ]);
        assert_eq!(parser.parse().unwrap(), expected);
    }

    #[test]
    fn display_nested_list() {
        let expr = SExpr::List(vec![
            SExpr::Atom("outer".to_string()),
            SExpr::List(vec![
                SExpr::Atom("inner".to_string()),
                SExpr::Atom("value".to_string()),
            ]),
        ]);
        assert_eq!(expr.to_string(), "(outer (inner value))");
    }

    #[test]
    fn display_empty_list() {
        let expr = SExpr::List(vec![]);
        assert_eq!(expr.to_string(), "()");
    }

    #[test]
    fn display_single_element_list() {
        let expr = SExpr::List(vec![SExpr::Atom("only".to_string())]);
        assert_eq!(expr.to_string(), "(only)");
    }

    #[test]
    fn parser_handles_carriage_return() {
        let mut parser = Parser::new("(a\r\nb)");
        assert_eq!(
            parser.parse().unwrap(),
            SExpr::List(vec![
                SExpr::Atom("a".to_string()),
                SExpr::Atom("b".to_string()),
            ])
        );
    }

    #[test]
    fn unclosed_nested_list_error() {
        let mut parser = Parser::new("(a (b (c)");
        let err = parser.parse().unwrap_err();
        assert!(err.to_string().contains("unclosed-list"));
    }

    #[test]
    fn quoted_string_empty() {
        let mut parser = Parser::new(r#""""#);
        assert_eq!(parser.parse().unwrap(), SExpr::Atom(r#""""#.to_string()));
    }

    #[test]
    fn escape_string_backslash() {
        assert_eq!(escape_string("a\\b"), "a\\\\b");
    }

    #[test]
    fn escape_string_quote() {
        assert_eq!(escape_string("a\"b"), "a\\\"b");
    }

    #[test]
    fn escape_string_newline() {
        assert_eq!(escape_string("a\nb"), "a\\nb");
    }

    #[test]
    fn escape_string_carriage_return() {
        assert_eq!(escape_string("a\rb"), "a\\rb");
    }

    #[test]
    fn escape_string_tab() {
        assert_eq!(escape_string("a\tb"), "a\\tb");
    }

    #[test]
    fn escape_string_combined() {
        assert_eq!(escape_string("a\"\n\\b"), "a\\\"\\n\\\\b");
    }

    #[test]
    fn string_atom_basic() {
        assert_eq!(string_atom("hello"), SExpr::Atom("\"hello\"".to_string()));
    }

    #[test]
    fn string_atom_with_escapes() {
        assert_eq!(
            string_atom("hello\nworld"),
            SExpr::Atom("\"hello\\nworld\"".to_string())
        );
    }

    #[test]
    fn extract_string_quoted() {
        let atom = SExpr::Atom("\"hello\"".to_string());
        assert_eq!(extract_string(&atom), "hello");
    }

    #[test]
    fn extract_string_unquoted() {
        let atom = SExpr::Atom("unquoted".to_string());
        assert_eq!(extract_string(&atom), "unquoted");
    }

    #[test]
    fn extract_string_with_escapes() {
        let atom = SExpr::Atom("\"hello\\nworld\"".to_string());
        assert_eq!(extract_string(&atom), "hello\nworld");
    }

    #[test]
    fn extract_string_from_list() {
        let list = SExpr::List(vec![SExpr::Atom("tag".to_string())]);
        assert_eq!(extract_string(&list), "");
    }

    #[test]
    fn error_new_creates_base_structure() {
        let err = SError::new("test-phase");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test-phase".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_with_code() {
        let err = SError::new("parse").with_code("syntax-error");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("parse".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("code".to_string()),
                    SExpr::Atom("syntax-error".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_with_message() {
        let err = SError::new("eval").with_message("Something went wrong");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("eval".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("message".to_string()),
                    SExpr::Atom("\"Something went wrong\"".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_with_atom_field() {
        let err = SError::new("test").with_atom_field("count", 42);
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("count".to_string()),
                    SExpr::Atom("42".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_with_string_field() {
        let err = SError::new("test").with_string_field("name", "value");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("name".to_string()),
                    SExpr::Atom("\"value\"".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_with_field_sexpr() {
        let field_value = SExpr::List(vec![
            SExpr::Atom("nested".to_string()),
            SExpr::Atom("data".to_string()),
        ]);
        let err = SError::new("test").with_field("complex", field_value.clone());
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![SExpr::Atom("complex".to_string()), field_value,]),
            ])
        );
    }

    #[test]
    fn error_chained_builders() {
        let err = SError::new("mutations")
            .with_code("index-out-of-bounds")
            .with_message("Path index exceeds list length")
            .with_atom_field("index", 10)
            .with_atom_field("list_length", 5);
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("mutations".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("code".to_string()),
                    SExpr::Atom("index-out-of-bounds".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("message".to_string()),
                    SExpr::Atom("\"Path index exceeds list length\"".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("index".to_string()),
                    SExpr::Atom("10".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("list_length".to_string()),
                    SExpr::Atom("5".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_into_detail_consumes() {
        let err = SError::new("test").with_code("test-code");
        let expected = err.detail().clone();
        let detail = err.into_detail();
        assert_eq!(detail, expected);
    }

    #[test]
    fn error_from_sexpr() {
        let sexpr = SExpr::List(vec![
            SExpr::Atom("error".to_string()),
            SExpr::Atom("custom".to_string()),
        ]);
        let err: SError = sexpr.clone().into();
        assert_eq!(*err.detail(), sexpr);
    }

    #[test]
    fn error_display_matches_detail() {
        let err = SError::new("test").with_code("code");
        assert_eq!(err.to_string(), err.detail().to_string());
    }

    #[test]
    fn error_escapes_newline_in_message() {
        let err = SError::new("test").with_message("line1\nline2");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("message".to_string()),
                    SExpr::Atom("\"line1\\nline2\"".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_escapes_tab_in_message() {
        let err = SError::new("test").with_message("col1\tcol2");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("message".to_string()),
                    SExpr::Atom("\"col1\\tcol2\"".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_escapes_quotes_in_message() {
        let err = SError::new("test").with_message("has \"quotes\"");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("message".to_string()),
                    SExpr::Atom("\"has \\\"quotes\\\"\"".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_escapes_backslash_in_message() {
        let err = SError::new("test").with_message("path\\to\\file");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("message".to_string()),
                    SExpr::Atom("\"path\\\\to\\\\file\"".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_escapes_carriage_return_in_message() {
        let err = SError::new("test").with_message("line1\rline2");
        assert_eq!(
            *err.detail(),
            SExpr::List(vec![
                SExpr::Atom("error".to_string()),
                SExpr::List(vec![
                    SExpr::Atom("phase".to_string()),
                    SExpr::Atom("test".to_string()),
                ]),
                SExpr::List(vec![
                    SExpr::Atom("message".to_string()),
                    SExpr::Atom("\"line1\\rline2\"".to_string()),
                ]),
            ])
        );
    }

    #[test]
    fn error_implements_std_error() {
        let err = SError::new("test");
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn error_clone() {
        let err = SError::new("test").with_code("code");
        let cloned = err.clone();
        assert_eq!(err.detail(), cloned.detail());
    }

    #[test]
    fn error_partial_eq() {
        let err1 = SError::new("test").with_code("code");
        let err2 = SError::new("test").with_code("code");
        let err3 = SError::new("test").with_code("different");
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
