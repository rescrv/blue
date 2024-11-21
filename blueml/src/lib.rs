#[allow(unused_variables)]
mod grammar;
mod lexer;

pub use lexer::{Lexer, LexicalError, Location, Token, TokenType};

#[derive(Clone, Debug)]
pub enum Error {
    InternalError(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Assignment(Atom, Data),
    Block(Block),
    AtomData(Atom, Data),
    AtomDictionary(Atom, Dictionary),
    Atom(Atom),
    DataData(Data, Data),
    DataDictionary(Data, Dictionary),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Atom {
    atom: String,
}

impl Atom {
    pub fn identifier(&self) -> &str {
        &self.atom
    }
}

impl TryFrom<Token> for Atom {
    type Error = Error;

    fn try_from(token: Token) -> Result<Self, Self::Error> {
        match token {
            Token::Atom(s) => Ok(Atom { atom: s }),
            _ => Err(Error::InternalError(
                "Token cannot be converted to Atom".to_string(),
            )),
        }
    }
}

impl TryFrom<Token> for String {
    type Error = Error;

    fn try_from(token: Token) -> Result<Self, Self::Error> {
        match token {
            Token::SingleQuotedString(s) => Ok(s),
            Token::TripleQuotedString(s) => Ok(s),
            _ => Err(Error::InternalError(
                "Token cannot be converted to Atom".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Data {
    Variable(String),
    SingleString(String),
    TripleString(String),
    F64(f64),
    Dictionary(Dictionary),
}

impl Eq for Data {}

impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Data::Variable(a), Data::Variable(b)) => a == b,
            (Data::SingleString(a), Data::SingleString(b)) => a == b,
            (Data::TripleString(a), Data::TripleString(b)) => a == b,
            (Data::F64(a), Data::F64(b)) => a.total_cmp(b).is_eq(),
            (Data::Dictionary(a), Data::Dictionary(b)) => a == b,
            _ => false,
        }
    }
}

impl TryFrom<Token> for Data {
    type Error = Error;

    fn try_from(token: Token) -> Result<Self, Self::Error> {
        match token {
            Token::SingleQuotedString(s) => Ok(Data::SingleString(s)),
            Token::TripleQuotedString(s) => Ok(Data::TripleString(s)),
            Token::F64(f) => Ok(Data::F64(f)),
            _ => Err(Error::InternalError(
                "Token cannot be converted to Data".to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dictionary {
    pub items: Vec<Statement>,
}

impl From<Dictionary> for Data {
    fn from(dictionary: Dictionary) -> Self {
        Data::Dictionary(dictionary)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub r#type: Atom,
    pub label: Option<String>,
    pub dict: Dictionary,
}

pub fn parse(
    input: &str,
) -> Result<Vec<Statement>, lalrpop_util::ParseError<Location, Token, LexicalError>> {
    let lexer = Lexer::new(input);
    grammar::StatementsParser::new().parse(lexer)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use crate::Statement;

    use super::*;

    #[test]
    fn end_to_end() {
        let input = r#"workload "foo" {
    workload "subfoo1" {
        query: 0.95,
        write: 0.05,
    }
    workload "subfoo2" {
        query: 0.5,
        write: 0.5,
    }
    "subfoo1": 0.1,
    "subfoo2": 0.9,
}
composition "bar" {
    0.1: {
        query: 0.95,
        write: 0.05,
    }
    0.9: {
        query: 0.5,
        write: 0.5,
    }
}
"#;
        let expected: Vec<Statement> = vec![
            Statement::Block(Block {
                r#type: Atom {
                    atom: "workload".to_string(),
                },
                label: Some("foo".to_string()),
                dict: Dictionary {
                    items: vec![
                        Statement::Block(Block {
                            r#type: Atom {
                                atom: "workload".to_string(),
                            },
                            label: Some("subfoo1".to_string()),
                            dict: Dictionary {
                                items: vec![
                                    Statement::AtomData(
                                        Atom {
                                            atom: "query".to_string(),
                                        },
                                        Data::F64(0.95),
                                    ),
                                    Statement::AtomData(
                                        Atom {
                                            atom: "write".to_string(),
                                        },
                                        Data::F64(0.05),
                                    ),
                                ],
                            },
                        }),
                        Statement::Block(Block {
                            r#type: Atom {
                                atom: "workload".to_string(),
                            },
                            label: Some("subfoo2".to_string()),
                            dict: Dictionary {
                                items: vec![
                                    Statement::AtomData(
                                        Atom {
                                            atom: "query".to_string(),
                                        },
                                        Data::F64(0.5),
                                    ),
                                    Statement::AtomData(
                                        Atom {
                                            atom: "write".to_string(),
                                        },
                                        Data::F64(0.5),
                                    ),
                                ],
                            },
                        }),
                        Statement::DataData(
                            Data::SingleString("subfoo1".to_string()),
                            Data::F64(0.1),
                        ),
                        Statement::DataData(
                            Data::SingleString("subfoo2".to_string()),
                            Data::F64(0.9),
                        ),
                    ],
                },
            }),
            Statement::Block(Block {
                r#type: Atom {
                    atom: "composition".to_string(),
                },
                label: Some("bar".to_string()),
                dict: Dictionary {
                    items: vec![
                        Statement::DataDictionary(
                            Data::F64(0.1),
                            Dictionary {
                                items: vec![
                                    Statement::AtomData(
                                        Atom {
                                            atom: "query".to_string(),
                                        },
                                        Data::F64(0.95),
                                    ),
                                    Statement::AtomData(
                                        Atom {
                                            atom: "write".to_string(),
                                        },
                                        Data::F64(0.05),
                                    ),
                                ],
                            },
                        ),
                        Statement::DataDictionary(
                            Data::F64(0.9),
                            Dictionary {
                                items: vec![
                                    Statement::AtomData(
                                        Atom {
                                            atom: "query".to_string(),
                                        },
                                        Data::F64(0.5),
                                    ),
                                    Statement::AtomData(
                                        Atom {
                                            atom: "write".to_string(),
                                        },
                                        Data::F64(0.5),
                                    ),
                                ],
                            },
                        ),
                    ],
                },
            }),
        ];
        let returned: Vec<Statement> = crate::parse(input).unwrap();
        println!("{:#?}", returned);
        assert_eq!(expected, returned);
    }
}
