//! error_core is a default implementation of [zerror::Z].

use std::backtrace::Backtrace;
use std::fmt::Debug;

use biometrics::Counter;

use prototk_derive::Message;

use zerror::Z;

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

#[derive(Clone, Debug, Default, Message)]
struct Internals {
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
}

/// [ErrorCore] implements 100% of Z for easy error reporting.  It's intended that people will wrap
/// and proxy ErrorCore and then implement a short summary on top that descends from an error enum.
#[derive(Clone, Debug, Default, Message)]
pub struct ErrorCore {
    internals: Box<Internals>,
}

impl ErrorCore {
    /// Create a new ErrorCore with the provided email and short summary.  The provided counter
    /// will be clicked each time a new error is created, to give people insight into the error.
    /// It's advisable to have a separate counter for different conditions.
    pub fn new(email: &str, short: &str, counter: &'static Counter) -> Self {
        counter.click();
        let backtrace = format!("{}", Backtrace::force_capture());
        let internals = Internals {
            email: email.to_owned(),
            short: short.to_owned(),
            backtrace,
            toks: Vec::new(),
            urls: Vec::new(),
            vars: Vec::new(),
        };
        Self {
            internals: Box::new(internals),
        }
    }
}

impl Z for ErrorCore {
    type Error = Self;

    fn long_form(&self) -> String {
        let mut s = String::default();
        s += &format!("{}\n\nOWNER: {}", self.internals.short, self.internals.email);
        for token in self.internals.toks.iter() {
            s += &format!("\n{}: {}", token.identifier, token.value);
        }
        for url in self.internals.urls.iter() {
            s += &format!("\n{}: {}", url.identifier, url.url);
        }
        if !self.internals.vars.is_empty() {
            s += "\n";
            for variable in self.internals.vars.iter() {
                s += &format!("\n{} = {}", variable.identifier, variable.value);
            }
        }
        s += &format!("\n\nbacktrace:\n{}", self.internals.backtrace);
        s
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.set_token(identifier, value);
        self
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        self.internals.toks.push(Token {
            identifier: identifier.to_owned(),
            value: value.to_owned(),
        });
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.internals.urls.push(Url {
            identifier: identifier.to_owned(),
            url: url.to_owned(),
        });
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.internals.vars.push(Variable {
            identifier: variable.to_owned(),
            value: format!("{:?}", x),
        });
    }
}
