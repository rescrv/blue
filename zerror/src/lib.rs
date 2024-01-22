#![doc = include_str!("../README.md")]

use std::fmt::Debug;

///////////////////////////////////////////////// Z ////////////////////////////////////////////////

/// The core type of zerror.  Implement this trait, or wrap and proxy ErrorCore, to create rich
/// errors in the long_form.  This integrates with the error handling "monad" over Result<T, Z>.
pub trait Z {
    /// The type of error returned from the with_* methods.
    type Error;

    /// Convert an error to a string free from "="*80.
    fn long_form(&self) -> String;

    /// Add a token.
    #[deprecated(since="0.4.0", note="use with_info instead")]
    fn with_token(self, identifier: &str, value: &str) -> Self::Error;
    /// Add a URL.
    #[deprecated(since="0.4.0", note="use with_info instead")]
    fn with_url(self, identifier: &str, url: &str) -> Self::Error;
    /// Add debug formatting of a local variable.
    #[deprecated(since="0.4.0", note="use with_info instead")]
    fn with_variable<X: Debug>(self, variable: &str, x: X) -> Self::Error;

    /// Add debug formatting of a local variable.
    fn with_info<X: Debug>(self, name: &str, value: X) -> Self::Error;
    /// Add debug formatting using a closure.
    fn with_lazy_info<F: FnOnce() -> String>(self, name: &str, value: F) -> Self::Error;
}

impl<T, E: Z<Error = E>> Z for Result<T, E> {
    type Error = Result<T, E>;

    fn long_form(&self) -> String {
        match self {
            Ok(_) => {
                panic!("called long_form() on Ok Result");
            }
            Err(e) => e.long_form(),
        }
    }

    #[allow(deprecated)]
    fn with_token(self, identifier: &str, value: &str) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_token(identifier, value)),
        }
    }

    #[allow(deprecated)]
    fn with_url(self, identifier: &str, url: &str) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_url(identifier, url)),
        }
    }

    #[allow(deprecated)]
    fn with_variable<X: Debug>(self, variable: &str, x: X) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_variable(variable, x)),
        }
    }

    fn with_info<X: Debug>(self, name: &str, value: X) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_info(name, value)),
        }
    }

    fn with_lazy_info<F: FnOnce() -> String>(self, name: &str, value: F) -> Self::Error {
        match self {
            Ok(_) => self,
            Err(e) => Err(e.with_info(name, value())),
        }
    }
}

/// Create an IoToZ trait that gets implemented for `Result<T, $error>` where `$error` is the macro
/// arg.
#[macro_export]
macro_rules! iotoz {
    ($error:ident) => {
        /// IO to Z.
        pub trait IoToZ<T>: Sized
        where
            $error: Z,
        {
            /// Convert the error into the result type.
            #[allow(clippy::wrong_self_convention)]
            fn as_z(self) -> Result<T, $error>;
            /// A pretty unwrap method.
            fn pretty_unwrap(self) -> T;
        }

        impl<T, E: Into<$error>> IoToZ<T> for Result<T, E> {
            fn as_z(self) -> Result<T, $error> {
                match self {
                    Ok(t) => Ok(t),
                    Err(e) => Err(e.into()),
                }
            }

            fn pretty_unwrap(self) -> T {
                match self {
                    Ok(t) => t,
                    Err(err) => {
                        let err: $error = err.into();
                        panic!("{}", err.long_form());
                    }
                }
            }
        }
    };
}
