//! Parser for the caternary concatenative language.
//!
//! The language consists of whitespace-separated words and bracketed expressions.
//! Words are interpreted exactly as written (case preserved). Bracketed expressions
//! are parsed recursively.

/// A span in source text, measured in byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Inclusive starting byte offset.
    pub start: usize,
    /// Exclusive ending byte offset.
    pub end: usize,
}

impl Span {
    fn point(byte: usize) -> Self {
        Self {
            start: byte,
            end: byte + 1,
        }
    }
}

/// A token in the caternary language.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// A non-whitespace word token.
    Word(String),
    /// A bracketed expression containing nested tokens.
    Bracket(Vec<Token>),
}

/// A token paired with the source span that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpannedToken {
    /// Source location for this token.
    pub span: Span,
    /// Token payload.
    pub kind: SpannedTokenKind,
}

/// Variant payload for [`SpannedToken`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpannedTokenKind {
    /// A non-whitespace word token.
    Word(String),
    /// A bracketed expression containing nested spanned tokens.
    Bracket(Vec<SpannedToken>),
}

impl SpannedToken {
    fn into_token(self) -> Token {
        match self.kind {
            SpannedTokenKind::Word(word) => Token::Word(word),
            SpannedTokenKind::Bracket(inner) => {
                Token::Bracket(inner.into_iter().map(SpannedToken::into_token).collect())
            }
        }
    }
}

/// An error that can occur during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// An opening `[` was not closed.
    UnmatchedOpenBracket {
        /// Span of the unmatched opening bracket.
        span: Span,
    },
    /// A closing `]` was encountered without a corresponding opening bracket.
    UnmatchedCloseBracket {
        /// Span of the unmatched closing bracket.
        span: Span,
    },
}

impl ParseError {
    /// Returns the source span associated with this parse error.
    pub fn span(&self) -> Span {
        match self {
            ParseError::UnmatchedOpenBracket { span } => *span,
            ParseError::UnmatchedCloseBracket { span } => *span,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnmatchedOpenBracket { span } => {
                write!(f, "unmatched opening bracket '[' at byte {}", span.start)
            }
            ParseError::UnmatchedCloseBracket { span } => {
                write!(f, "unmatched closing bracket ']' at byte {}", span.start)
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses a caternary program from source text.
pub fn parse(input: &str) -> Result<Vec<Token>, ParseError> {
    Ok(parse_with_spans(input)?
        .into_iter()
        .map(SpannedToken::into_token)
        .collect())
}

/// Parses a caternary program and returns tokens with source spans.
pub fn parse_with_spans(input: &str) -> Result<Vec<SpannedToken>, ParseError> {
    let mut parser = Parser::new(input);
    parser.parse_tokens(None)
}

struct Parser<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    fn parse_tokens(&mut self, open_bracket: Option<usize>) -> Result<Vec<SpannedToken>, ParseError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();

            let Some((offset, ch)) = self.peek_char() else {
                return match open_bracket {
                    Some(byte) => Err(ParseError::UnmatchedOpenBracket {
                        span: Span::point(byte),
                    }),
                    None => Ok(tokens),
                };
            };

            match ch {
                '[' => {
                    self.bump_char();
                    let inner = self.parse_tokens(Some(offset))?;
                    tokens.push(SpannedToken {
                        span: Span {
                            start: offset,
                            end: self.cursor,
                        },
                        kind: SpannedTokenKind::Bracket(inner),
                    });
                }
                ']' => {
                    if open_bracket.is_some() {
                        self.bump_char();
                        return Ok(tokens);
                    }
                    return Err(ParseError::UnmatchedCloseBracket {
                        span: Span::point(offset),
                    });
                }
                _ => {
                    let (word, span) = self.parse_word();
                    tokens.push(SpannedToken {
                        span,
                        kind: SpannedTokenKind::Word(word),
                    });
                }
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some((_, ch)) = self.peek_char() {
            if ch.is_whitespace() {
                self.bump_char();
            } else {
                break;
            }
        }
    }

    fn parse_word(&mut self) -> (String, Span) {
        let start = self.cursor;
        let mut word = String::new();
        while let Some((_, ch)) = self.peek_char() {
            if ch.is_whitespace() || ch == '[' || ch == ']' {
                break;
            }
            word.push(ch);
            self.bump_char();
        }
        (
            word,
            Span {
                start,
                end: self.cursor,
            },
        )
    }

    fn peek_char(&self) -> Option<(usize, char)> {
        self.input[self.cursor..]
            .char_indices()
            .next()
            .map(|(idx, ch)| (self.cursor + idx, ch))
    }

    fn bump_char(&mut self) -> Option<char> {
        let (_, ch) = self.peek_char()?;
        self.cursor += ch.len_utf8();
        Some(ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_preserve_case() {
        let tokens = parse("Scan SCAN scan").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("Scan".to_string()),
                Token::Word("SCAN".to_string()),
                Token::Word("scan".to_string()),
            ]
        );
    }

    #[test]
    fn nested_brackets() {
        let tokens = parse("[a [b c] d]").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Bracket(vec![
                Token::Word("a".to_string()),
                Token::Bracket(vec![
                    Token::Word("b".to_string()),
                    Token::Word("c".to_string()),
                ]),
                Token::Word("d".to_string()),
            ])]
        );
    }

    #[test]
    fn span_for_word_and_bracket() {
        let tokens = parse_with_spans("aa [bbb]").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].span, Span { start: 0, end: 2 });
        assert_eq!(tokens[1].span, Span { start: 3, end: 8 });
    }

    #[test]
    fn unmatched_open_bracket_has_span() {
        let err = parse("[x").unwrap_err();
        assert!(matches!(err, ParseError::UnmatchedOpenBracket { .. }));
        assert_eq!(err.span(), Span { start: 0, end: 1 });
        assert_eq!(
            err.to_string(),
            "unmatched opening bracket '[' at byte 0".to_string()
        );
    }

    #[test]
    fn unmatched_close_bracket_has_span() {
        let err = parse("x ]").unwrap_err();
        assert!(matches!(err, ParseError::UnmatchedCloseBracket { .. }));
        assert_eq!(err.span(), Span { start: 2, end: 3 });
        assert_eq!(
            err.to_string(),
            "unmatched closing bracket ']' at byte 2".to_string()
        );
    }
}
