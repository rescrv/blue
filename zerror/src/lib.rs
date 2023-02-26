#[allow(unused_imports)]
#[macro_use]
extern crate prototk_derive;

use std::backtrace::Backtrace;
use std::collections::btree_map::BTreeMap;
use std::error::Error;
use std::fmt::Debug;

use biometrics::Counter;

pub const VALUE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER;
pub const BACKTRACE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 1;
pub const NESTED_ERROR_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 2;
pub const NESTED_ZERROR_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 3;

///////////////////////////////////////////////// Z ////////////////////////////////////////////////

pub trait Z {
    type Error;

    // Convert an error to a string free from "="*80.
    fn as_utf8(&self) -> String;

    // What caused this error.
    fn source(&self) -> Option<&(dyn Error + 'static)>;
    // Set the source that caused the error.
    fn set_source<E: Error + 'static>(&mut self, err: E);

    // Add a token.
    fn with_token(self, identifier: &str, value: &str) -> Self::Error;
    fn set_token(&mut self, identifier: &str, value: &str);
    // Add a URL.
    fn with_url(self, identifier: &str, url: &str) -> Self::Error;
    fn set_url(&mut self, identifier: &str, url: &str);
    // Add debug formatting of a local variable.
    fn with_variable<X: Debug>(self, variable: &str, x: X) -> Self::Error;
    fn set_variable<X: Debug>(&mut self, variable: &str, x: X);
}

impl<T, E: Z<Error=E>> Z for Result<T, E> {
    type Error = Result<T, E>;

    fn as_utf8(&self) -> String {
        match self {
            Ok(_) => {
                panic!("called \"<Result<T, E> as Z>.as_utf8()\" on Ok Result");
            },
            Err(e) => { e.as_utf8() },
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

#[derive(Debug, Default)]
pub struct ErrorCore {
    email: String,
    short: String,
    backtrace: String,
    toks: BTreeMap<String, String>,
    urls: Vec<(String, String)>,
    vars: Vec<(String, String)>,
    source: Option<Box<dyn Error + 'static>>,
}

impl ErrorCore {
    pub fn new(email: &str, short: &str, counter: &'static Counter) -> Self {
        counter.click();
        let backtrace = format!("{}", Backtrace::force_capture());
        Self {
            email: email.to_owned(),
            short: short.to_owned(),
            backtrace,
            toks: BTreeMap::new(),
            urls: Vec::new(),
            vars: Vec::new(),
            source: None,
        }
    }
}

impl Z for ErrorCore {
    type Error = Self;

    fn as_utf8(&self) -> String {
        let mut s = String::default();
        s += &format!("{}\n\nOWNER: {}", self.short, self.email);
        for (key, val) in self.toks.iter() {
            s += &format!("\n{}: {}", key, val);
        }
        for (key, val) in self.urls.iter() {
            s += &format!("\n{}: {}", key, val);
        }
        if !self.vars.is_empty() {
            s += &format!("\n");
            for (key, val) in self.vars.iter() {
                s += &format!("\n{} = {}", key, val);
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
        self.toks.insert(identifier.to_owned(), value.to_owned());
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.urls.push((identifier.to_owned(), url.to_owned()));
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.vars.push((variable.to_owned(), format!("{:?}", x)));
    }
}
