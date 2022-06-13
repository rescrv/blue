extern crate prototk;
#[macro_use]
extern crate prototk_derive;

pub mod block;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    BlockError { what: block::Error },
}

impl From<block::Error> for Error {
    fn from(what: block::Error) -> Error {
        Error::BlockError {
            what,
        }
    }
}

/////////////////////////////////////////// KeyValuePair ///////////////////////////////////////////

#[derive(Debug, Eq, PartialEq)]
pub struct KeyValuePair<'a> {
    pub key: &'a [u8],
    pub timestamp: u64,
    pub value: Option<&'a [u8]>,
}

///////////////////////////////////////////// Iterator /////////////////////////////////////////////

pub trait Iterator {
    // Seek functions should not return a value, but instead position the cursor so that a
    // subsequent call to next or prev will return the right result.  For example, seek_to_first
    // should position the cursor so that a prev returns None and a next returns the first result.
    // A seek should position the cursor so that a call to next will return the result, while a
    // call to prev will return the previous result.
    fn seek_to_first(&mut self) -> Result<(), Error>;
    fn seek_to_last(&mut self) -> Result<(), Error>;
    fn seek(&mut self, key: &[u8]) -> Result<(), Error>;

    fn prev(&mut self) -> Result<Option<KeyValuePair>, Error>;
    fn next(&mut self) -> Result<Option<KeyValuePair>, Error>;
    fn same(&mut self) -> Result<Option<KeyValuePair>, Error>;
}
