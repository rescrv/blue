#[allow(unused_imports)]
#[macro_use]
extern crate prototk_derive;

use std::backtrace::Backtrace;
use std::fmt::{Debug, Display, Formatter};

use buffertk::{stack_pack, Packable};

use prototk::field_types::*;
use prototk::{FieldNumber, FieldType, Tag};

pub const VALUE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER;
pub const BACKTRACE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 1;
pub const NESTED_ERROR_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 2;
pub const NESTED_ZERROR_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 3;

////////////////////////////////////////////// ZError //////////////////////////////////////////////

pub struct ZError<E: Clone + Debug + Display> {
    error: E,
    proto: Vec<u8>,
    human: String,
    source: Option<Box<dyn std::error::Error + 'static>>,
}

impl<E: Clone + Debug + Display> ZError<E> {
    pub fn new<'a, F: FieldType<'a, NativeType=E>>(error: E) -> Self {
        let tag = Tag {
            field_number: FieldNumber::must(VALUE_FIELD_NUMBER),
            wire_type: F::WIRE_TYPE,
        };
        let proto = stack_pack(tag).pack(F::from_native(error.clone())).to_vec();
        Self {
            error,
            proto: proto,
            human: String::default(),
            source: None,
        }
        .with_backtrace()
    }

    pub fn wrap_zerror<F: Clone + Debug + Display + 'static>(mut self, wrapped: ZError<F>) -> Self {
        let wrapped_str = format!("{}", wrapped);
        self = self
            .with_human("wrapped", wrapped_str)
            .with_protobuf::<bytes>(NESTED_ZERROR_FIELD_NUMBER, &wrapped.proto);
        self.source = Some(Box::new(wrapped));
        self
    }

    pub fn wrap_error(mut self, wrapped: Box<dyn std::error::Error + 'static>) -> Self {
        let wrapped_str = format!("{}", wrapped);
        self.source = Some(wrapped);
        self.with_context::<string>("wrapped", NESTED_ERROR_FIELD_NUMBER, wrapped_str)
    }

    pub fn with_context<'a, F: FieldType<'a>>(self, field_name: &str, field_number: u32, field_value: F::NativeType) -> Self
    where
        F::NativeType: Clone + Display,
    {
        self.with_protobuf::<F>(field_number, field_value.clone())
            .with_human::<F::NativeType>(field_name, field_value)
    }

    pub fn with_human<'a, F: Display>(mut self, field_name: &str, field_value: F) -> Self {
        self.human = format!("{} = {}\n", field_name, field_value) + &self.human;
        self
    }

    pub fn with_protobuf<'a, F: FieldType<'a>>(mut self, field_number: u32, field_value: F::NativeType) -> Self {
        let tag = Tag {
            field_number: FieldNumber::must(field_number),
            wire_type: F::WIRE_TYPE,
        };
        let field = F::from_native(field_value);
        stack_pack(tag).pack(field).append_to_vec(&mut self.proto);
        self
    }

    pub fn with_backtrace(self) -> Self {
        let backtrace = format!("{}", Backtrace::force_capture());
        self.with_context::<string>("backtrace", BACKTRACE_FIELD_NUMBER, backtrace)
    }
}

impl<E: Clone + Debug + Display> AsRef<E> for ZError<E> {
    fn as_ref(&self) -> &E {
        &self.error
    }
}

impl<E: Clone + Debug + Display> std::error::Error for ZError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(x) => Some(x.as_ref()),
            None => None,
        }
    }
}

impl<E: Clone + Debug + Display> Display for ZError<E> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}\n{}", self.error, self.human)?;
        if let Some(nested) = &self.source {
            write!(fmt, "\n{}", nested)?;
        }
        Ok(())
    }
}

impl<E: Clone + Debug + Display> Debug for ZError<E> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{:?}\n{}", self.error, self.human)?;
        if let Some(nested) = &self.source {
            write!(fmt, "\n{:?}", nested)?;
        }
        Ok(())
    }
}

impl<E: Clone + Debug + Display> Packable for ZError<E> {
    fn pack_sz(&self) -> usize {
        self.proto.len()
    }

    fn pack(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.proto);
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn new_has_backtrace() {
        let zerr: ZError<u32> = ZError::new::<fixed32>(5);
        assert_eq!(5, *zerr.as_ref());
        let exp: &[u8] = &[253, 255, 255, 255, 15, 5, 0, 0, 0, 242, 255, 255, 255, 15];
        assert_eq!(exp, &zerr.proto[..14]);
        assert!(zerr.human.starts_with("backtrace = "));
    }

    #[test]
    fn with_protobuf() {
        let mut zerr: ZError<u32> = ZError::new::<fixed32>(5);
        // test reset
        zerr.proto = Vec::new();
        zerr.human = "".to_owned();
        // body
        zerr = zerr.with_protobuf::<stringref>(1, "this string");
        let exp: &[u8] = &[10, 11, 116, 104, 105, 115, 32, 115, 116, 114, 105, 110, 103];
        assert_eq!(exp, zerr.proto);
    }

    #[test]
    fn with_human() {
        let mut zerr: ZError<u32> = ZError::new::<fixed32>(5);
        // test reset
        zerr.proto = Vec::new();
        zerr.human = "".to_owned();
        // body
        zerr = zerr.with_human("test_string", "this string");
        assert_eq!("test_string = this string\n", zerr.human);
    }

    #[test]
    fn with_context() {
        let mut zerr: ZError<u32> = ZError::new::<fixed32>(5);
        // test reset
        zerr.proto = Vec::new();
        zerr.human = "".to_owned();
        zerr = zerr.with_context::<stringref>("test_string", 1, "this string");
        // proto
        let exp: &[u8] = &[10, 11, 116, 104, 105, 115, 32, 115, 116, 114, 105, 110, 103];
        assert_eq!(exp, zerr.proto);
        // human
        assert_eq!("test_string = this string\n", zerr.human);
    }

    #[test]
    fn wrap_error() {
        let wrapped: Box<dyn Error + 'static> = "wrapped error".into();
        let zerr: ZError<&'static str> = ZError::new::<stringref>("wrapping error").wrap_error(wrapped);
        // proto
        let exp: &[u8] = &[234, 255, 255, 255, 15, 13, 119, 114, 97, 112, 112, 101, 100, 32, 101, 114, 114, 111, 114];
        assert_eq!(exp, &zerr.proto[zerr.proto.len()-exp.len()..]);
        // human
        assert!(zerr.human.starts_with("wrapped = wrapped error\n"));
    }

    #[test]
    fn wrap_zerror() {
        let wrapped: ZError<&'static str> = ZError::new::<stringref>("wrapped error");
        let zerr: ZError<&'static str> = ZError::new::<stringref>("wrapping error").wrap_zerror(wrapped);
        // look for "wrapping error"
        let exp: &[u8] = &[250, 255, 255, 255, 15, 14, 119, 114, 97, 112, 112, 105, 110, 103, 32, 101, 114, 114, 111, 114];
        assert_eq!(exp, &zerr.proto[..20]);
        // find an offset
        let exp: &[u8] = &[250, 255, 255, 255, 15, 13, 119, 114, 97, 112, 112, 101, 100, 32, 101, 114, 114, 111, 114];
        for idx in 0..zerr.proto.len()-exp.len() {
            if exp == &zerr.proto[idx..idx+exp.len()] {
                return;
            }
        }
        let got: &[u8] = &zerr.proto;
        assert_eq!(exp, got);
        panic!("test didn't find wrapped error");
    }

    #[derive(Clone, Debug, Message)]
    enum TestError {
        #[prototk(1, string)]
        DefaultError(String),
        #[prototk(2, string)]
        Case1(String),
        #[prototk(3, string)]
        Case2(String),
    }

    impl Default for TestError {
        fn default() -> Self {
            TestError::DefaultError("default".to_owned())
        }
    }

    impl Display for TestError {
        fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
            write!(fmt, "{:?}", self)
        }
    }

    impl TestError {
        fn default_error(s: String) -> ZError<TestError> {
            ZError::new::<message<TestError>>(TestError::DefaultError(s))
        }

        fn case1(s: String) -> ZError<TestError> {
            ZError::new::<message<TestError>>(TestError::Case1(s))
        }

        fn case2(s: String) -> ZError<TestError> {
            ZError::new::<message<TestError>>(TestError::Case2(s))
        }
    }

    #[test]
    fn message() {
        let _ = TestError::default_error("default".to_string());
        let _ = TestError::case1("1".to_string());
        let _ = TestError::case2("2".to_string());
    }
}
