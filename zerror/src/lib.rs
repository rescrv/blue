//! zerror is a module for creating rich error types.

use std::backtrace::Backtrace;
use std::error::Error;
use std::fmt::Debug;

use biometrics::Counter;

use prototk_derive::Message;

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

///////////////////////////////////////////// ErrorCore ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
struct Token {
    #[prototk(1, string)]
    identifier: String,
    #[prototk(2, string)]
    value: String,
}

#[derive(Clone, Debug, Default, Message)]
struct Url {
    #[prototk(1, string)]
    identifier: String,
    #[prototk(2, string)]
    url: String,
}

#[derive(Clone, Debug, Default, Message)]
struct Variable {
    #[prototk(1, string)]
    identifier: String,
    #[prototk(2, string)]
    value: String,
}

/// [ErrorCore] implements 100% of Z for easy error reporting.  It's intended that people will wrap
/// and proxy ErrorCore and then implement a short summary on top that descends from an error enum.
#[derive(Debug, Default, Message)]
pub struct ErrorCore {
    #[prototk(1, string)]
    email: String,
    #[prototk(2, string)]
    short: String,
    #[prototk(3, string)]
    backtrace: String,
    #[prototk(4, message)]
    toks: Vec<Token>,
    #[prototk(5, message)]
    urls: Vec<Url>,
    #[prototk(6, message)]
    vars: Vec<Variable>,
    source: Option<Box<dyn Error + 'static>>,
}

impl ErrorCore {
    /// Create a new ErrorCore with the provided email and short summary.  The provided counter
    /// will be clicked each time a new error is created, to give people insight into the error.
    /// It's advisable to have a separate counter for different conditions.
    pub fn new(email: &str, short: &str, counter: &'static Counter) -> Self {
        counter.click();
        let backtrace = format!("{}", Backtrace::force_capture());
        Self {
            email: email.to_owned(),
            short: short.to_owned(),
            backtrace,
            toks: Vec::new(),
            urls: Vec::new(),
            vars: Vec::new(),
            source: None,
        }
    }
}

impl Z for ErrorCore {
    type Error = Self;

    fn long_form(&self) -> String {
        let mut s = String::default();
        s += &format!("{}\n\nOWNER: {}", self.short, self.email);
        for token in self.toks.iter() {
            s += &format!("\n{}: {}", token.identifier, token.value);
        }
        for url in self.urls.iter() {
            s += &format!("\n{}: {}", url.identifier, url.url);
        }
        if !self.vars.is_empty() {
            s += "\n";
            for variable in self.vars.iter() {
                s += &format!("\n{} = {}", variable.identifier, variable.value);
            }
        }
        s += &format!("\n\nbacktrace:\n{}", self.backtrace);
        if let Some(source) = &self.source {
            s += &format!("\n\nsource error:\n{}", source);
        }
        s
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.source {
            Some(e) => { Some(e.as_ref()) },
            None => { None },
        }
    }

    fn set_source<E: Error + 'static>(&mut self, err: E) {
        self.source = Some(Box::new(err));
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.set_token(identifier, value);
        self
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        self.toks.push(Token { identifier: identifier.to_owned(), value: value.to_owned() });
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.urls.push(Url { identifier: identifier.to_owned(), url: url.to_owned() });
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.vars.push(Variable { identifier: variable.to_owned(), value: format!("{:?}", x) });
    }
}
