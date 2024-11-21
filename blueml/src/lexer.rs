use std::fmt::Display;
use std::iter::Peekable;
use std::str::Chars;

#[derive(Clone, Debug)]
pub enum LexicalError {}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TokenType {
    Percent,
    Equals,
    Colon,
    Comma,
    DollarSign,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Atom,
    SingleQuotedString,
    TripleQuotedString,
    F64,
    Comment,
}

#[derive(Clone, Debug)]
pub enum Token {
    Percent,
    Equals,
    Colon,
    Comma,
    DollarSign,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Atom(String),
    SingleQuotedString(String),
    TripleQuotedString(String),
    F64(f64),
    Comment(String),
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Percent => write!(f, "%"),
            Token::Equals => write!(f, "="),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::DollarSign => write!(f, "$"),
            Token::LeftBrace => write!(f, "{{"),
            Token::RightBrace => write!(f, "}}"),
            Token::LeftBracket => write!(f, "["),
            Token::RightBracket => write!(f, "]"),
            Token::Atom(s) => write!(f, "{}", s),
            Token::SingleQuotedString(s) => write!(f, r#""{}""#, escape(s)),
            Token::TripleQuotedString(s) => write!(f, r#""""{}""""#, s),
            Token::F64(x) => write!(f, "{}", x),
            Token::Comment(s) => write!(f, "{}", s),
        }
    }
}

impl Eq for Token {}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Token::Percent, Token::Percent)
            | (Token::Equals, Token::Equals)
            | (Token::Colon, Token::Colon)
            | (Token::Comma, Token::Comma)
            | (Token::DollarSign, Token::DollarSign)
            | (Token::LeftBrace, Token::LeftBrace)
            | (Token::RightBrace, Token::RightBrace)
            | (Token::LeftBracket, Token::LeftBracket)
            | (Token::RightBracket, Token::RightBracket) => true,
            (Token::Atom(s1), Token::Atom(s2)) => s1 == s2,
            (Token::SingleQuotedString(s1), Token::SingleQuotedString(s2)) => s1 == s2,
            (Token::TripleQuotedString(s1), Token::TripleQuotedString(s2)) => s1 == s2,
            (Token::F64(x1), Token::F64(x2)) => x1.total_cmp(x2).is_eq(),
            (Token::Comment(s1), Token::Comment(s2)) => s1 == s2,
            _ => false,
        }
    }
}

impl From<Token> for TokenType {
    fn from(token: Token) -> Self {
        match token {
            Token::Percent => TokenType::Percent,
            Token::Equals => TokenType::Equals,
            Token::Colon => TokenType::Colon,
            Token::Comma => TokenType::Comma,
            Token::DollarSign => TokenType::DollarSign,
            Token::LeftBrace => TokenType::LeftBrace,
            Token::RightBrace => TokenType::RightBrace,
            Token::LeftBracket => TokenType::LeftBracket,
            Token::RightBracket => TokenType::RightBracket,
            Token::Atom(_) => TokenType::Atom,
            Token::SingleQuotedString(_) => TokenType::SingleQuotedString,
            Token::TripleQuotedString(_) => TokenType::TripleQuotedString,
            Token::F64(_) => TokenType::F64,
            Token::Comment(_) => TokenType::Comment,
        }
    }
}

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
    current: Option<Token>,
    offset: usize,
    line: usize,
    column: usize,
    primed: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input: input.chars().peekable(),
            current: None,
            offset: 1,
            line: 1,
            column: 1,
            primed: false,
        }
    }

    pub fn advance(&mut self) {
        self.primed = false;
        self.current = None;
    }

    pub fn current(&mut self) -> Option<Token> {
        if !self.primed {
            self.prime();
        }
        self.current.clone()
    }

    fn prime(&mut self) {
        assert!(!self.primed);
        assert!(self.current.is_none());
        // Eat up any whitespace to get to the first character that can be part of a token.
        loop {
            let Some(c) = self.peek() else {
                return;
            };
            if !c.is_whitespace() {
                break;
            }
            self.next();
        }
        let Some(token) = self.peek() else {
            return;
        };
        match token {
            ',' => {
                self.next();
                self.current = Some(Token::Comma);
            }
            '$' => {
                self.next();
                self.current = Some(Token::DollarSign);
            }
            '=' => {
                self.next();
                self.current = Some(Token::Equals);
            }
            ':' => {
                self.next();
                self.current = Some(Token::Colon);
            }
            '{' => {
                self.next();
                self.current = Some(Token::LeftBrace);
            }
            '}' => {
                self.next();
                self.current = Some(Token::RightBrace);
            }
            '[' => {
                self.next();
                self.current = Some(Token::LeftBracket);
            }
            ']' => {
                self.next();
                self.current = Some(Token::RightBracket);
            }
            '%' => {
                self.next();
                self.current = Some(Token::Percent);
            }
            '-' | '+' | '0'..='9' => {
                let mut f = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit()
                        || (c == '-' && f.is_empty())
                        || (c == '+' && f.is_empty())
                        || (c == '.' && !f.contains('.') && !f.contains('e'))
                        || (c == 'e' && !f.contains('e'))
                    {
                        f.push(c);
                        self.next();
                    } else if c.is_ascii_punctuation() || c.is_whitespace() {
                        break;
                    } else {
                        todo!();
                    }
                }
                if let Ok(f) = f.parse::<f64>() {
                    self.current = Some(Token::F64(f));
                } else {
                    todo!();
                }
            }
            '"' => {
                let mut s = String::new();
                while let Some(c) = self.next() {
                    s.push(c);
                    if parse_triple_string(&s).is_some()
                        || (parse_single_string(&s).is_some()
                            && (s.chars().count() > 2 || self.peek() != Some('"')))
                    {
                        break;
                    }
                }
                if let Some(s) = parse_triple_string(&s) {
                    self.current = Some(Token::TripleQuotedString(s));
                } else if let Some(s) = parse_single_string(&s) {
                    self.current = Some(Token::SingleQuotedString(s));
                } else if self.peek().is_some() {
                    todo!();
                } else {
                    todo!();
                }
            }
            '#' => {
                let mut s = String::new();
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    s.push(c);
                    self.next();
                }
                self.current = Some(Token::Comment(s));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut s = String::new();
                loop {
                    let Some(c) = self.peek() else {
                        break;
                    };
                    if !s.is_empty()
                        && !c.is_alphanumeric()
                        && c != '_'
                        && (c.is_ascii_punctuation() || c.is_whitespace())
                    {
                        break;
                    }
                    s.push(c);
                    self.next();
                }
                self.current = Some(Token::Atom(s));
            }
            _ => {
                todo!("{}", token);
            }
        }
        self.primed = true;
    }

    fn next(&mut self) -> Option<char> {
        let c = self.input.next();
        if let Some(c) = c {
            self.offset += 1;
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some(c)
        } else {
            None
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.input.peek().cloned()
    }
}

///////////////////////////////////////////// Location /////////////////////////////////////////////

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Location {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl Default for Location {
    fn default() -> Self {
        Location {
            offset: 1,
            line: 1,
            column: 1,
        }
    }
}

/////////////////////////////////////////////// utils //////////////////////////////////////////////

fn escape(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '\n' => "\\n".to_string(),
            '\\' => "\\\\".to_string(),
            '"' => "\\\"".to_string(),
            '`' => "\\`".to_string(),
            '$' => "\\$".to_string(),
            c => c.to_string(),
        })
        .collect()
}

fn parse_triple_string(input: &str) -> Option<String> {
    if !input.starts_with(r#"""""#) {
        return None;
    }
    if !input.ends_with(r#"""""#) {
        return None;
    }
    if input.chars().count() < 6 {
        return None;
    }
    let s = input
        .chars()
        .skip(3)
        .take(input.chars().count() - 6)
        .collect::<String>();
    if s.contains(r#"""""#) {
        return None;
    }
    Some(s)
}

fn parse_single_string(input: &str) -> Option<String> {
    if !input.starts_with('"') {
        return None;
    }
    if !input.ends_with('"') {
        return None;
    }
    if input.chars().count() < 2 {
        return None;
    }
    let mut output = String::new();
    let mut prev_was_whack = false;
    for c in input.chars().skip(1).take(input.chars().count() - 2) {
        match c {
            '$' if prev_was_whack => {
                output.push('$');
                prev_was_whack = false;
            }
            '`' if prev_was_whack => {
                output.push('`');
                prev_was_whack = false;
            }
            '"' if prev_was_whack => {
                output.push('"');
                prev_was_whack = false;
            }
            '\\' if prev_was_whack => {
                output.push('\\');
                prev_was_whack = false;
            }
            'n' if prev_was_whack => {
                output.push('\n');
                prev_was_whack = false;
            }
            '\n' if prev_was_whack => {
                output.push(' ');
                prev_was_whack = false;
            }
            '"' => {
                return None;
            }
            '\\' => {
                prev_was_whack = true;
            }
            '$' => {}
            c if prev_was_whack => {
                output.push('\\');
                output.push(c);
                prev_was_whack = false;
            }
            c => {
                output.push(c);
                prev_was_whack = false;
            }
        }
    }
    if prev_was_whack {
        None
    } else {
        Some(output)
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<(Location, Token, Location), LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = Location {
            offset: self.offset,
            line: self.line,
            column: self.column,
        };
        self.advance();
        let token = self.current();
        let limit = Location {
            offset: self.offset,
            line: self.line,
            column: self.column,
        };
        token.map(|t| Ok((start, t, limit)))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn single_quoted() {
        assert_eq!(Some("".to_string()), parse_single_string(r#""""#));
        assert_eq!(Some("\"".to_string()), parse_single_string(r#""\"""#));
    }

    fn token_type_strategy() -> impl Strategy<Value = Token> {
        prop_oneof![
            Just(Token::Percent),
            Just(Token::DollarSign),
            Just(Token::LeftBrace),
            Just(Token::RightBrace),
            Just(Token::LeftBracket),
            Just(Token::RightBracket),
            "[a-z][a-zA-Z0-9]".prop_map(Token::Atom),
            any::<String>().prop_map(Token::SingleQuotedString),
            any::<String>()
                .prop_filter(
                    "triple quoted strings must start or end with quotes and cannot contain triple quotes",
                    |s| !s.starts_with('"') && !s.ends_with('"') && !s.contains(r#"""""#)
                )
                .prop_map(Token::TripleQuotedString),
            any::<f64>().prop_map(Token::F64),
            // NOTE(rescrv):  Comments aren't expected to survive the round trip because we will
            // join the comment with a space between comment and newline.  Therefore, we don't test
            // comment lexing with proptests.
        ]
    }

    proptest::proptest! {
        #[test]
        fn lexer_round_trip_whitespace(expected in proptest::collection::vec(token_type_strategy(), 0..100)) {
            let input = expected.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ");
            println!("input:  {:?}", input);
            println!("expected: {:?}", expected);
            let mut lexer = Lexer::new(&input);
            let mut returned = vec![];
            while let Some(token) = lexer.current() {
                returned.push(token.clone());
                lexer.advance();
            }
            println!("returned: {:?}", returned);
            assert_eq!(expected, returned);
        }
    }
}
