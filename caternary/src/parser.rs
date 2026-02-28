//! Parser for the caternary concatenative language.
//!
//! The language consists of whitespace-separated tokens:
//! - Words: uppercase identifiers like `SCAN`, `FILTER`, `BUILD`
//! - Bracketed expressions: `[...]` which can contain arbitrary content including nested brackets
//!
//! Bracketed expressions are parsed recursively but their interpretation is deferred to runtime.

use std::iter::Peekable;
use std::str::Chars;

/// A token in the caternary language.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    /// A word (identifier), typically uppercase.
    Word(String),
    /// A bracketed expression containing a sequence of tokens.
    Bracket(Vec<Token>),
}

/// An error that can occur during parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Unmatched opening bracket.
    UnmatchedOpenBracket,
    /// Unmatched closing bracket.
    UnmatchedCloseBracket,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnmatchedOpenBracket => write!(f, "unmatched opening bracket '['"),
            ParseError::UnmatchedCloseBracket => write!(f, "unmatched closing bracket ']'"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses a caternary program from source text.
pub fn parse(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut chars = input.chars().peekable();
    parse_tokens(&mut chars, false)
}

fn parse_tokens(chars: &mut Peekable<Chars>, in_bracket: bool) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();

    loop {
        skip_whitespace(chars);

        match chars.peek() {
            None => {
                if in_bracket {
                    return Err(ParseError::UnmatchedOpenBracket);
                }
                return Ok(tokens);
            }
            Some('[') => {
                chars.next();
                let inner = parse_tokens(chars, true)?;
                tokens.push(Token::Bracket(inner));
            }
            Some(']') => {
                if in_bracket {
                    chars.next();
                    return Ok(tokens);
                } else {
                    return Err(ParseError::UnmatchedCloseBracket);
                }
            }
            Some(_) => {
                let word = parse_word(chars);
                if !word.is_empty() {
                    tokens.push(Token::Word(word));
                }
            }
        }
    }
}

fn skip_whitespace(chars: &mut Peekable<Chars>) {
    while let Some(c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn parse_word(chars: &mut Peekable<Chars>) -> String {
    let mut word = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '[' || c == ']' {
            break;
        }
        word.push(c);
        chars.next();
    }
    word
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_words() {
        let tokens = parse("A SCAN FILTER").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("A".to_string()),
                Token::Word("SCAN".to_string()),
                Token::Word("FILTER".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn bracketed_expression() {
        let tokens = parse("[foo < 5] FILTER").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Bracket(vec![
                    Token::Word("foo".to_string()),
                    Token::Word("<".to_string()),
                    Token::Word("5".to_string()),
                ]),
                Token::Word("FILTER".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn nested_brackets() {
        let tokens = parse("[C SCAN [bar && baz] FILTER] OPTIMIZE EXPLAIN").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Bracket(vec![
                    Token::Word("C".to_string()),
                    Token::Word("SCAN".to_string()),
                    Token::Bracket(vec![
                        Token::Word("bar".to_string()),
                        Token::Word("&&".to_string()),
                        Token::Word("baz".to_string()),
                    ]),
                    Token::Word("FILTER".to_string()),
                ]),
                Token::Word("OPTIMIZE".to_string()),
                Token::Word("EXPLAIN".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn full_example() {
        let input = r#"
            A SCAN [foo < 5] FILTER
            B SCAN [foo > 3] FILTER
            BUILD
            PROBE
        "#;
        let tokens = parse(input).unwrap();
        assert_eq!(tokens.len(), 10);
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn unmatched_open_bracket() {
        let result = parse("[foo");
        assert_eq!(result, Err(ParseError::UnmatchedOpenBracket));
        println!("Error: {:?}", result);
    }

    #[test]
    fn unmatched_close_bracket() {
        let result = parse("foo]");
        assert_eq!(result, Err(ParseError::UnmatchedCloseBracket));
        println!("Error: {:?}", result);
    }

    #[test]
    fn empty_input() {
        let tokens = parse("").unwrap();
        assert!(tokens.is_empty());
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn empty_brackets() {
        let tokens = parse("[] FILTER").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Bracket(vec![]), Token::Word("FILTER".to_string()),]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn single_word() {
        let tokens = parse("WORD").unwrap();
        assert_eq!(tokens, vec![Token::Word("WORD".to_string())]);
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn whitespace_only() {
        let tokens = parse("   \t\n  ").unwrap();
        assert!(tokens.is_empty());
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn multiple_spaces_between_words() {
        let tokens = parse("A    B\t\tC\n\nD").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("A".to_string()),
                Token::Word("B".to_string()),
                Token::Word("C".to_string()),
                Token::Word("D".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn brackets_without_spaces() {
        let tokens = parse("[A][B]C").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Bracket(vec![Token::Word("A".to_string())]),
                Token::Bracket(vec![Token::Word("B".to_string())]),
                Token::Word("C".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn word_immediately_before_bracket() {
        let tokens = parse("WORD[inner]").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("WORD".to_string()),
                Token::Bracket(vec![Token::Word("inner".to_string())]),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn word_immediately_after_bracket() {
        let tokens = parse("[inner]WORD").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Bracket(vec![Token::Word("inner".to_string())]),
                Token::Word("WORD".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn deeply_nested_brackets() {
        let tokens = parse("[[[A]]]").unwrap();
        assert_eq!(
            tokens,
            vec![Token::Bracket(vec![Token::Bracket(vec![Token::Bracket(
                vec![Token::Word("A".to_string())]
            )])])]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn special_characters_in_words() {
        let tokens = parse("foo->bar a.b.c $var @attr #tag").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("foo->bar".to_string()),
                Token::Word("a.b.c".to_string()),
                Token::Word("$var".to_string()),
                Token::Word("@attr".to_string()),
                Token::Word("#tag".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn operators_as_words() {
        let tokens = parse("+ - * / < > <= >= == != && ||").unwrap();
        assert_eq!(tokens.len(), 12);
        assert_eq!(tokens[0], Token::Word("+".to_string()));
        assert_eq!(tokens[10], Token::Word("&&".to_string()));
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn unmatched_nested_open_bracket() {
        let result = parse("[[A]");
        assert_eq!(result, Err(ParseError::UnmatchedOpenBracket));
        println!("Error: {:?}", result);
    }

    #[test]
    fn unmatched_nested_close_bracket() {
        let result = parse("[A]]");
        assert_eq!(result, Err(ParseError::UnmatchedCloseBracket));
        println!("Error: {:?}", result);
    }

    #[test]
    fn close_bracket_in_middle() {
        let result = parse("A ] B");
        assert_eq!(result, Err(ParseError::UnmatchedCloseBracket));
        println!("Error: {:?}", result);
    }

    #[test]
    fn unicode_words() {
        let tokens = parse("héllo wörld 日本語").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("héllo".to_string()),
                Token::Word("wörld".to_string()),
                Token::Word("日本語".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn numbers_as_words() {
        let tokens = parse("123 45.67 -89").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Word("123".to_string()),
                Token::Word("45.67".to_string()),
                Token::Word("-89".to_string()),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn empty_nested_brackets() {
        let tokens = parse("[[]]").unwrap();
        assert_eq!(tokens, vec![Token::Bracket(vec![Token::Bracket(vec![])])]);
        println!("Parsed: {:?}", tokens);
    }

    #[test]
    fn multiple_brackets_same_level() {
        let tokens = parse("[A B] [C D] [E F]").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Bracket(vec![
                    Token::Word("A".to_string()),
                    Token::Word("B".to_string())
                ]),
                Token::Bracket(vec![
                    Token::Word("C".to_string()),
                    Token::Word("D".to_string())
                ]),
                Token::Bracket(vec![
                    Token::Word("E".to_string()),
                    Token::Word("F".to_string())
                ]),
            ]
        );
        println!("Parsed: {:?}", tokens);
    }
}
