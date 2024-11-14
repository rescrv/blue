mod grammar;
mod lexer;

pub use lexer::{Lexer, Token, TokenType};

#[derive(Clone, Debug)]
pub enum Error {
    InternalError(String),
}

#[derive(Clone, Debug)]
pub enum Type {
    Assignment(Atom, Data),
    AtomBlock(Atom, Block),
    AtomData(Atom, Data),
    Atom(Atom),
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub enum Data {
    Variable(String),
    SingleString(String),
    TripleString(String),
    F64(f64),
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

#[derive(Clone, Debug)]
pub struct Block {
    pub types: Vec<Type>,
    pub datas: Vec<(AtomOrData, AtomOrData)>,
}

#[derive(Clone, Debug)]
pub enum AtomOrData {
    Atom(Atom),
    Data(Data),
}

pub fn parse(content: &str) -> Result<Vec<Type>, Error> {
    let lexer = Lexer::new(content);
    let data = grammar::TypesParser::new().parse(lexer).unwrap();
    Ok(data)
}
