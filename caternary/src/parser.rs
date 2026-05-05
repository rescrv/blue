//! Parser for the caternary concatenative language.
//!
//! The language first uses shell tokenization and then applies FORTH-style
//! quotation parsing to the resulting words. Words are interpreted exactly as
//! written after shell splitting (case preserved). Bracketed expressions are
//! parsed recursively from `[` and `]` characters in those shell-split words.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitState {
    Unquoted,
    Single,
    Double,
}

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
    /// Shell tokenization failed before FORTH processing began.
    Tokenization {
        /// Human-readable tokenization error.
        message: String,
    },
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
            ParseError::Tokenization { .. } => Span { start: 0, end: 0 },
            ParseError::UnmatchedOpenBracket { span } => *span,
            ParseError::UnmatchedCloseBracket { span } => *span,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Tokenization { message } => {
                write!(f, "shell tokenization failed: {message}")
            }
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
    let mut parser = Parser::new();
    for shell_word in shell_words(input)? {
        parser.process_shell_word(shell_word)?;
    }
    parser.finish()
}

struct ShellWord {
    text: String,
    span: Span,
    emitted: Vec<EmittedChar>,
}

#[derive(Clone, Copy)]
struct EmittedChar {
    ch: char,
    span: Span,
}

struct Frame {
    open_bracket: Option<usize>,
    tokens: Vec<SpannedToken>,
}

struct Parser {
    frames: Vec<Frame>,
}

impl Parser {
    fn new() -> Self {
        Self {
            frames: vec![Frame {
                open_bracket: None,
                tokens: Vec::new(),
            }],
        }
    }

    fn process_shell_word(&mut self, shell_word: ShellWord) -> Result<(), ParseError> {
        if shell_word.text.is_empty() {
            self.push_word(String::new(), shell_word.span);
            return Ok(());
        }

        let mut segment_start = shell_word.span.start;
        let mut segment = String::new();

        for emitted in shell_word.emitted {
            match emitted.ch {
                '[' => {
                    self.flush_segment(&mut segment, segment_start, emitted.span.start);
                    self.frames.push(Frame {
                        open_bracket: Some(emitted.span.start),
                        tokens: Vec::new(),
                    });
                    segment_start = emitted.span.end;
                }
                ']' => {
                    self.flush_segment(&mut segment, segment_start, emitted.span.start);
                    let Some(frame) = self.frames.pop() else {
                        unreachable!("parser always has a root frame");
                    };
                    let Some(open_start) = frame.open_bracket else {
                        self.frames.push(frame);
                        return Err(ParseError::UnmatchedCloseBracket { span: emitted.span });
                    };
                    self.current_tokens().push(SpannedToken {
                        span: Span {
                            start: open_start,
                            end: emitted.span.end,
                        },
                        kind: SpannedTokenKind::Bracket(frame.tokens),
                    });
                    segment_start = emitted.span.end;
                }
                ch => {
                    segment.push(ch);
                }
            }
        }

        self.flush_segment(&mut segment, segment_start, shell_word.span.end);
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<SpannedToken>, ParseError> {
        if let Some(frame) = self.frames.pop() {
            if let Some(open_start) = frame.open_bracket {
                return Err(ParseError::UnmatchedOpenBracket {
                    span: Span::point(open_start),
                });
            }
            return Ok(frame.tokens);
        }
        unreachable!("parser always has a root frame");
    }

    fn current_tokens(&mut self) -> &mut Vec<SpannedToken> {
        &mut self
            .frames
            .last_mut()
            .expect("parser always has a root frame")
            .tokens
    }

    fn flush_segment(&mut self, segment: &mut String, start: usize, end: usize) {
        if !segment.is_empty() {
            let word = std::mem::take(segment);
            self.push_word(word, Span { start, end });
        }
    }

    fn push_word(&mut self, word: String, span: Span) {
        self.current_tokens().push(SpannedToken {
            span,
            kind: SpannedTokenKind::Word(word),
        });
    }
}

fn shell_words(input: &str) -> Result<Vec<ShellWord>, ParseError> {
    let split = shvar::split(input).map_err(|err| ParseError::Tokenization {
        message: err.to_string(),
    })?;
    let spans = shell_word_spans(input).map_err(|err| ParseError::Tokenization {
        message: err.to_string(),
    })?;
    debug_assert_eq!(split.len(), spans.len());

    let mut shell_words = Vec::with_capacity(split.len());
    for (text, span) in split.into_iter().zip(spans.into_iter()) {
        let emitted = decode_shell_word(&input[span.start..span.end], span.start);
        debug_assert_eq!(text, emitted.iter().map(|c| c.ch).collect::<String>());
        shell_words.push(ShellWord {
            text,
            span,
            emitted,
        });
    }
    Ok(shell_words)
}

fn shell_word_spans(input: &str) -> Result<Vec<Span>, shvar::Error> {
    let mut spans = Vec::new();
    let mut remaining = input;

    while let Some((_, rest)) = shvar::split_once(remaining)? {
        let base = input.len() - remaining.len();
        let start = base + leading_whitespace_bytes(remaining);
        let end = input.len() - rest.len();
        spans.push(Span { start, end });
        remaining = rest;
    }

    Ok(spans)
}

fn leading_whitespace_bytes(input: &str) -> usize {
    input
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum()
}

fn decode_shell_word(input: &str, base: usize) -> Vec<EmittedChar> {
    let mut emitted = Vec::new();
    let mut state = SplitState::Unquoted;
    let mut whack_start = None;

    for (idx, ch) in input.char_indices() {
        let start = base + idx;
        let end = start + ch.len_utf8();
        match (state, ch) {
            (SplitState::Double, '$') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '$',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Double, '`') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '`',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Double, '"') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '"',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Double, '\\') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '\\',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Double, '\n') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '\n',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Double, 'n') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '\n',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Double, '"') => {
                state = SplitState::Unquoted;
            }
            (SplitState::Double, '\\') => {
                whack_start = Some(start);
            }
            (SplitState::Double, c) if whack_start.is_some() => {
                let whack = whack_start.take().unwrap_or(start);
                emitted.push(EmittedChar {
                    ch: '\\',
                    span: Span {
                        start: whack,
                        end: whack + 1,
                    },
                });
                emitted.push(EmittedChar {
                    ch: c,
                    span: Span { start, end },
                });
            }
            (SplitState::Double, c) => {
                emitted.push(EmittedChar {
                    ch: c,
                    span: Span { start, end },
                });
            }
            (SplitState::Single, '\'') => {
                state = SplitState::Unquoted;
            }
            (SplitState::Single, c) => {
                emitted.push(EmittedChar {
                    ch: c,
                    span: Span { start, end },
                });
            }
            (SplitState::Unquoted, c) if c.is_whitespace() && whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: c,
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Unquoted, '\'') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '\'',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Unquoted, '\'') => {
                state = SplitState::Single;
            }
            (SplitState::Unquoted, '"') if whack_start.is_some() => {
                emitted.push(EmittedChar {
                    ch: '"',
                    span: Span {
                        start: whack_start.take().unwrap_or(start),
                        end,
                    },
                });
            }
            (SplitState::Unquoted, '"') => {
                state = SplitState::Double;
            }
            (SplitState::Unquoted, '\\') if whack_start.is_none() => {
                whack_start = Some(start);
            }
            (SplitState::Unquoted, c) if whack_start.is_some() => {
                let whack = whack_start.take().unwrap_or(start);
                emitted.push(EmittedChar {
                    ch: '\\',
                    span: Span {
                        start: whack,
                        end: whack + 1,
                    },
                });
                emitted.push(EmittedChar {
                    ch: c,
                    span: Span { start, end },
                });
            }
            (SplitState::Unquoted, c) => {
                emitted.push(EmittedChar {
                    ch: c,
                    span: Span { start, end },
                });
            }
        }
    }

    emitted
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
    fn shell_quoted_words_preserve_spaces() {
        let tokens = parse(r#""hello world" ['a b']"#).unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("hello world".to_string()),
                Token::Bracket(vec![Token::Word("a b".to_string())]),
            ]
        );
    }

    #[test]
    fn compact_brackets_round_trip_shell_quoted_words() {
        let tokens = parse(r#"["hello world" ECHO]CALL"#).unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Bracket(vec![
                    Token::Word("hello world".to_string()),
                    Token::Word("ECHO".to_string()),
                ]),
                Token::Word("CALL".to_string()),
            ]
        );
    }

    #[test]
    fn escaped_whitespace_is_one_word() {
        let tokens = parse(r"hello\ world").unwrap();
        assert_eq!(tokens, vec![Token::Word("hello world".to_string())]);
    }

    #[test]
    fn empty_quoted_word_is_a_token() {
        let tokens = parse(r#""""#).unwrap();
        assert_eq!(tokens, vec![Token::Word(String::new())]);
    }

    #[test]
    fn span_for_word_and_bracket() {
        let tokens = parse_with_spans("aa [bbb]").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].span, Span { start: 0, end: 2 });
        assert_eq!(tokens[1].span, Span { start: 3, end: 8 });
    }

    #[test]
    fn quoted_word_span_includes_quotes() {
        let tokens = parse_with_spans(r#""aa bb""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].span, Span { start: 0, end: 7 });
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
