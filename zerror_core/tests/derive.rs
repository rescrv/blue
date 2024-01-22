use zerror::{iotoz, Z};
use zerror_core::ErrorCore;
use zerror_derive::Z;

#[derive(Z)]
pub enum SampleError {
    Success { core: ErrorCore },
    Failure { core: ErrorCore },
    FailureWithString { core: ErrorCore, what: String },
}

iotoz! {SampleError}

#[test]
fn core() {
    let err = SampleError::Success {
        core: ErrorCore::default(),
    };
    let _core = err.core();
    let err = SampleError::Failure {
        core: ErrorCore::default(),
    };
    let _core = err.core();
    let err = SampleError::FailureWithString {
        core: ErrorCore::default(),
        what: "Some string".to_owned(),
    };
    let _core = err.core();
}

#[test]
fn core_mut() {
    let mut err = SampleError::Success {
        core: ErrorCore::default(),
    };
    let _core = err.core_mut();
    let mut err = SampleError::Failure {
        core: ErrorCore::default(),
    };
    let _core = err.core_mut();
    let mut err = SampleError::FailureWithString {
        core: ErrorCore::default(),
        what: "Some string".to_owned(),
    };
    let _core = err.core_mut();
}
