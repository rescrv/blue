#![doc = include_str!("../README.md")]

use std::fmt::Debug;

////////////////////////////////////////////// Handle //////////////////////////////////////////////

pub trait Handle<T>
where
    Self: Sized,
{
    fn handle(&self) -> Option<T> {
        None
    }
}

impl<OK, ERR: Handle<T>, T> Handle<T> for Result<OK, ERR> {
    fn handle(&self) -> Option<T> {
        self.as_ref().err().and_then(|e| e.handle())
    }
}

/////////////////////////////////////////// HandleResult ///////////////////////////////////////////

pub trait HandleResult<T>: Handle<T> {
    fn handle_result(self) -> Result<Self, T>
    where
        Self: Sized,
    {
        if let Some(err) = self.handle() {
            Err(err)
        } else {
            Ok(self)
        }
    }
}

impl<OK, ERR: Handle<T>, T> HandleResult<T> for Result<OK, ERR> {}

////////////////////////////////////////// no_quote_debug //////////////////////////////////////////

pub fn no_quote_debug(s: &str) -> impl Debug {
    struct NoQuoteDebug(String);
    impl Debug for NoQuoteDebug {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
    NoQuoteDebug(s.to_string())
}
