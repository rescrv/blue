//! zerror is a module for creating rich error types.

use std::error::Error;
use std::fmt::Debug;

///////////////////////////////////////////////// Z ////////////////////////////////////////////////

/// The core type of zerror.  Implement this trait, or wrap and proxy ErrorCore, to create rich
/// errors in the long_form.  This integrates with the error handling "monad" over Result<T, Z>.
pub trait Z {
    type Error;

    /// Convert an error to a string free from "="*80.
    fn long_form(&self) -> String;

    /// What caused this error.
    fn source(&self) -> Option<&(dyn Error + 'static)>;
    /// Set the source that caused the error.
    fn set_source<E: Error + 'static>(&mut self, err: E);

    /// Add a token.
    fn with_token(self, identifier: &str, value: &str) -> Self::Error;
    /// Add a token.
    fn set_token(&mut self, identifier: &str, value: &str);
    /// Add a URL.
    fn with_url(self, identifier: &str, url: &str) -> Self::Error;
    /// Add a URL.
    fn set_url(&mut self, identifier: &str, url: &str);
    /// Add debug formatting of a local variable.
    fn with_variable<X: Debug>(self, variable: &str, x: X) -> Self::Error;
    /// Add debug formatting of a local variable.
    fn set_variable<X: Debug>(&mut self, variable: &str, x: X);
}

impl<T, E: Z<Error=E>> Z for Result<T, E> {
    type Error = Result<T, E>;

    fn long_form(&self) -> String {
        match self {
            Ok(_) => {
                panic!("called \"<Result<T, E> as Z>.long_form()\" on Ok Result");
            },
            Err(e) => { e.long_form() },
        }
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Ok(_) => { None },
            Err(e) => { e.source() },
        }
    }

    fn set_source<R: Error + 'static>(&mut self, err: R) {
        match self {
            Ok(_) => {
                panic!("called \"<Result<T, E> as Z>.set_source()\" on Ok Result");
            },
            Err(e) => { e.set_source(err) },
        }
    }

    fn with_token(self, identifier: &str, value: &str) -> Self::Error {
        match self {
            Ok(_) => { self },
            Err(e) => { Err(e.with_token(identifier, value)) },
        }
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        if let Err(e) = self {
            e.set_token(identifier, value);
        }
    }

    fn with_url(self, identifier: &str, url: &str) -> Self::Error {
        match self {
            Ok(_) => { self },
            Err(e) => { Err(e.with_url(identifier, url)) },
        }
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        if let Err(e) = self {
            e.set_url(identifier, url);
        }
    }

    fn with_variable<X: Debug>(self, variable: &str, x: X) -> Self::Error {
        match self {
            Ok(_) => { self },
            Err(e) => { Err(e.with_variable(variable, x)) },
        }
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        if let Err(e) = self {
            e.set_variable(variable, x);
        }
    }
}
