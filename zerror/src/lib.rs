#![doc = include_str!("../README.md")]

use std::fmt::Debug;

///////////////////////////////////////////////// Z ////////////////////////////////////////////////

/// The core type of zerror.  Implement this trait, or wrap and proxy ErrorCore, to create rich
/// errors in the long_form.  This integrates with the error handling "monad" over Result<T, Z>.
pub trait Z {
    type Error;

    /// Convert an error to a string free from "="*80.
    fn long_form(&self) -> String;

    /// Add a token.
    fn with_token(self, identifier: &str, value: &str) -> Self::Error;
    /// Add a URL.
    fn with_url(self, identifier: &str, url: &str) -> Self::Error;
    /// Add debug formatting of a local variable.
    fn with_variable<X: Debug>(self, variable: &str, x: X) -> Self::Error;
}

impl<T, E: Z<Error=E>> Z for Result<T, E> {
    type Error = Result<T, E>;

    fn long_form(&self) -> String {
        match self {
            Ok(_) => {
                panic!("called long_form() on Ok Result");
            },
            Err(e) => {
                e.long_form()
            },
        }
    }

    fn with_token(self, identifier: &str, value: &str) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_token(identifier, value)),
        }
    }

    fn with_url(self, identifier: &str, url: &str) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_url(identifier, url)),
        }
    }

    fn with_variable<X: Debug>(self, variable: &str, x: X) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_variable(variable, x)),
        }
    }
}

/// Create an IoToZ trait that gets implemented for `Result<T, $error>` where `$error` is the macro
/// arg.
#[macro_export]
macro_rules! iotoz {
    ($error:ident) => {
        pub trait IoToZ<T> {
            fn as_z(self) -> Result<T, $error>;
        }

        impl<T, E: Into<$error>> IoToZ<T> for Result<T, E> {
            fn as_z(self) -> Result<T, $error> {
                match self {
                    Ok(t) => Ok(t),
                    Err(e) => Err(e.into()),
                }
            }
        }
    };
}
