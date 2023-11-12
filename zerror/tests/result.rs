use std::fmt::Debug;

use zerror::{iotoz, Z};

#[derive(Debug)]
pub struct SampleError {
    err: std::io::Error,
    var: Vec<String>,
}

impl From<std::io::Error> for SampleError {
    fn from(err: std::io::Error) -> Self {
        Self {
            err,
            var: Vec::new(),
        }
    }
}

impl Z for SampleError {
    type Error = Self;

    fn long_form(&self) -> String {
        "long form".to_owned()
    }

    fn with_token(self, _identifier: &str, _value: &str) -> Self::Error {
        todo!();
    }

    fn with_url(self, _identifier: &str, _url: &str) -> Self::Error {
        todo!();
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error {
        self.var.push(format!("{}: {:?}", variable, x));
        self
    }
}

iotoz!(SampleError);

#[test]
fn sample_error() {
    let success: Result<(), std::io::Error> = Ok(());
    let failure: Result<(), std::io::Error> =
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "message"));

    let success: Result<(), SampleError> = success.as_z();
    let failure: Result<(), SampleError> = failure.as_z();

    let success = success.with_variable("TOKEN", 42);
    assert!(success.is_ok());

    let failure = failure.with_variable("TOKEN", 42);
    assert!(failure.is_err());
    if let Err(err) = failure {
        assert_eq!(vec!["TOKEN: 42"], err.var);
        assert_eq!("message", format!("{}", err.err));
    }
}
