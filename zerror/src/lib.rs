#[allow(unused_imports)]
#[macro_use]
extern crate prototk_derive;

use std::backtrace::Backtrace;
use std::fmt::{Debug, Display, Formatter};

use prototk::field_types::*;
use prototk::{FieldHelper, FieldType};
use prototk::Builder as ProtoTKBuilder;

pub const VALUE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER;
pub const BACKTRACE_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 1;
pub const NESTED_ERROR_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 2;
pub const NESTED_ZERROR_FIELD_NUMBER: u32 = prototk::LAST_FIELD_NUMBER - 3;

////////////////////////////////////////////// ZError //////////////////////////////////////////////

pub struct ZError<E: Debug + Display> {
    error: E,
    proto: ProtoTKBuilder,
    human: String,
    source: Option<Box<dyn std::error::Error + 'static>>,
}

impl<E: Debug + Display> ZError<E> {
    pub fn new<'a>(error: E) -> Self {
        Self {
            error,
            proto: ProtoTKBuilder::default(),
            human: String::default(),
            source: None,
        }
        .with_backtrace()
    }

    pub fn wrap_zerror<F: Clone + Debug + Display + 'static>(mut self, wrapped: ZError<F>) -> Self {
        let wrapped_str = format!("{}", wrapped);
        self = self
            .with_human("wrapped", wrapped_str)
            .with_protobuf::<bytes, NESTED_ZERROR_FIELD_NUMBER>(wrapped.proto.as_bytes());
        self.source = Some(Box::new(wrapped));
        self
    }

    pub fn wrap_error(mut self, wrapped: Box<dyn std::error::Error + 'static>) -> Self {
        let wrapped_str = format!("{}", wrapped);
        self.source = Some(wrapped);
        self.with_context::<string, NESTED_ERROR_FIELD_NUMBER>("wrapped", &wrapped_str)
    }

    pub fn with_context<'a, F: FieldType<'a> + 'a, const N: u32>(
        self,
        field_name: &str,
        field_value: F::NativeType,
    ) -> Self
    where
        F::NativeType: Clone + Display + FieldHelper<'a, F>,
    {
        self.with_protobuf::<F, N>(field_value.clone())
            .with_human::<F::NativeType>(field_name, field_value)
    }

    pub fn with_human<'a, F: Display>(mut self, field_name: &str, field_value: F) -> Self {
        self.human += &format!("{} = {}\n", field_name, field_value);
        self
    }

    pub fn with_protobuf<'a, F: FieldType<'a> + 'a, const N: u32>(
        mut self,
        field_value: F::NativeType,
    ) -> Self
    where
        F::NativeType: FieldHelper<'a, F>,
    {
        self.proto.push::<F, N>(field_value);
        self
    }

    pub fn with_backtrace(mut self) -> Self {
        let backtrace = format!("{}", Backtrace::force_capture());
        self = self.with_protobuf::<string, BACKTRACE_FIELD_NUMBER>(&backtrace);
        self.human += "backtrace:\n";
        self.human += &backtrace;
        self.human += "\n";
        self
    }

    pub fn to_proto(&self) -> Vec<u8> {
        self.proto.as_bytes().to_vec()
    }
}

impl<'a, E: Clone + Debug + Display + 'a> ZError<E> {
    pub fn value<F: FieldType<'a, NativeType = E> + 'a>(error: E) -> Self
    where
        E: FieldHelper<'a, F>,
    {
        let mut proto = ProtoTKBuilder::default();
        proto.push::<F, VALUE_FIELD_NUMBER>(error.clone());
        Self {
            error,
            proto,
            human: String::default(),
            source: None,
        }
        .with_backtrace()
    }
}

impl<E: Debug + Display> AsRef<E> for ZError<E> {
    fn as_ref(&self) -> &E {
        &self.error
    }
}

impl<E: Debug + Display> std::error::Error for ZError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.source {
            Some(x) => Some(x.as_ref()),
            None => None,
        }
    }
}

impl<E: Debug + Display> Display for ZError<E> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}\n{}", self.error, self.human)?;
        if let Some(nested) = &self.source {
            write!(fmt, "\n{}", nested)?;
        }
        Ok(())
    }
}

impl<E: Debug + Display> Debug for ZError<E> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{:?}\n{}", self.error, self.human)?;
        if let Some(nested) = &self.source {
            write!(fmt, "\n{:?}", nested)?;
        }
        Ok(())
    }
}

///////////////////////////////////////////// AsZerror /////////////////////////////////////////////

pub trait AsZError {
    type Error: Debug + Display + Sized;

    fn zerr(self) -> ZError<Self::Error>;
}

impl AsZError for std::io::Error {
    type Error = std::io::Error;

    fn zerr(self) -> ZError<Self::Error> {
        ZError::new(self)
    }
}

//////////////////////////////////////////// FromIOError ///////////////////////////////////////////

pub trait FromIOError<T, E: Debug + Display> {
    fn from_io(self) -> Result<T, ZError<E>>;
}

impl<T, E: Debug + Display + From<std::io::Error>> FromIOError<T, E> for Result<T, std::io::Error> {
    fn from_io(self) -> Result<T, ZError<E>> {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::new(E::from(e))),
        }
    }
}

/////////////////////////////////////////// ZErrorResult ///////////////////////////////////////////

pub trait ZErrorResult {
    type Error;

    fn wrap_zerror<E: Clone + Debug + Display + 'static>(self, wrapped: ZError<E>) -> Self::Error;

    fn wrap_error(self, wrapped: Box<dyn std::error::Error + 'static>) -> Self::Error;

    fn with_context<'a, F: FieldType<'a> + 'a, const N: u32>(
        self,
        field_name: &str,
        field_value: F::NativeType,
    ) -> Self::Error
    where
        <F as FieldType<'a>>::NativeType: Clone + Debug + Display + FieldHelper<'a, F>;

    fn with_human<'a, F: Display>(self, field_name: &str, field_value: F) -> Self::Error;

    fn with_protobuf<'a, F: FieldType<'a> + 'a, const N: u32>(
        self,
        field_value: F::NativeType,
    ) -> Self::Error
    where
        <F as FieldType<'a>>::NativeType: Clone + Debug + Display + FieldHelper<'a, F>;

    fn with_backtrace(self) -> Self::Error;
}

impl<T, E: Debug + Display> ZErrorResult for Result<T, ZError<E>> {
    type Error = Result<T, ZError<E>>;

    fn wrap_zerror<F: Clone + Debug + Display + 'static>(self, wrapped: ZError<F>) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::wrap_zerror(e, wrapped)),
        }
    }

    fn wrap_error(self, wrapped: Box<dyn std::error::Error + 'static>) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::wrap_error(e, wrapped)),
        }
    }

    fn with_context<'a, F: FieldType<'a> + 'a, const N: u32>(
        self,
        field_name: &str,
        field_value: F::NativeType,
    ) -> Self::Error
    where
        <F as FieldType<'a>>::NativeType: Clone + Debug + Display + FieldHelper<'a, F>,
    {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::with_context::<F, N>(
                e,
                field_name,
                field_value,
            )),
        }
    }

    fn with_human<'a, F: Display>(self, field_name: &str, field_value: F) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::with_human::<F>(e, field_name, field_value)),
        }
    }

    fn with_protobuf<'a, F: FieldType<'a> + 'a, const N: u32>(
        self,
        field_value: F::NativeType,
    ) -> Self::Error
    where
        <F as FieldType<'a>>::NativeType: Clone + Debug + Display + FieldHelper<'a, F>,
    {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::with_protobuf::<F, N>(e, field_value)),
        }
    }

    fn with_backtrace(self) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::with_backtrace(e)),
        }
    }
}

impl<T> ZErrorResult for Result<T, std::io::Error> {
    type Error = Result<T, ZError<std::io::Error>>;

    fn wrap_zerror<F: Clone + Debug + Display + 'static>(self, wrapped: ZError<F>) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::wrap_zerror(e.zerr(), wrapped)),
        }
    }

    fn wrap_error(self, wrapped: Box<dyn std::error::Error + 'static>) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::wrap_error(e.zerr(), wrapped)),
        }
    }

    fn with_context<'a, F: FieldType<'a> + 'a, const N: u32>(
        self,
        field_name: &str,
        field_value: F::NativeType,
    ) -> Self::Error
    where
        <F as FieldType<'a>>::NativeType: Clone + Debug + Display + FieldHelper<'a, F>,
    {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(e
                .zerr()
                .with_context::<F, N>(field_name, field_value)),
        }
    }

    fn with_human<'a, F: Display>(self, field_name: &str, field_value: F) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::with_human(e.zerr(), field_name, field_value)),
        }
    }

    fn with_protobuf<'a, F: FieldType<'a> + 'a, const N: u32>(
        self,
        field_value: F::NativeType,
    ) -> Self::Error
    where
        <F as FieldType<'a>>::NativeType: Clone + Debug + Display + FieldHelper<'a, F>,
    {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::with_protobuf::<F, N>(
                e.zerr(),
                field_value,
            )),
        }
    }

    fn with_backtrace(self) -> Self::Error {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(ZError::with_backtrace(e.zerr())),
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn new_has_backtrace() {
        let zerr: ZError<u32> = ZError::value::<fixed32>(5);
        assert_eq!(5, *zerr.as_ref());
        let exp: &[u8] = &[253, 255, 255, 255, 15, 5, 0, 0, 0, 242, 255, 255, 255, 15];
        assert_eq!(exp, &zerr.proto.as_bytes()[..14]);
        assert!(zerr.human.starts_with("backtrace:\n"));
    }

    #[test]
    fn with_protobuf() {
        let mut zerr: ZError<u32> = ZError::value::<fixed32>(5);
        // test reset
        zerr.proto = ProtoTKBuilder::default();
        zerr.human = "".to_owned();
        // body
        zerr = zerr.with_protobuf::<string, 1>("this string");
        let exp: &[u8] = &[10, 11, 116, 104, 105, 115, 32, 115, 116, 114, 105, 110, 103];
        assert_eq!(exp, zerr.proto.as_bytes());
    }

    #[test]
    fn with_human() {
        let mut zerr: ZError<u32> = ZError::value::<fixed32>(5);
        // test reset
        zerr.proto = ProtoTKBuilder::default();
        zerr.human = "".to_owned();
        // body
        zerr = zerr.with_human("test_string", "this string");
        assert_eq!("test_string = this string\n", zerr.human);
    }

    #[test]
    fn with_context() {
        let mut zerr: ZError<u32> = ZError::value::<fixed32>(5);
        // test reset
        zerr.proto = ProtoTKBuilder::default();
        zerr.human = "".to_owned();
        zerr = zerr.with_context::<string, 1>("test_string", "this string");
        // proto
        let exp: &[u8] = &[10, 11, 116, 104, 105, 115, 32, 115, 116, 114, 105, 110, 103];
        assert_eq!(exp, zerr.proto.as_bytes());
        // human
        assert_eq!("test_string = this string\n", zerr.human);
    }

    #[test]
    fn wrap_error() {
        let wrapped: Box<dyn Error + 'static> = "wrapped error".into();
        let zerr: ZError<&'static str> =
            ZError::value::<string>("wrapping error").wrap_error(wrapped);
        // proto
        let exp: &[u8] = &[];
        let got: &[u8] = zerr.proto.as_bytes();
        assert_eq!(exp, &got[got.len() - exp.len()..]);
        // human
        println!("zerr.human = {}", zerr.human);
        assert!(zerr.human.ends_with("wrapped = wrapped error\n"));
    }

    #[test]
    fn wrap_zerror() {
        let wrapped: ZError<&'static str> = ZError::value::<string>("wrapped error");
        let zerr: ZError<&'static str> =
            ZError::value::<string>("wrapping error").wrap_zerror(wrapped);
        // look for "wrapping error"
        let exp: &[u8] = &[
            250, 255, 255, 255, 15, 14, 119, 114, 97, 112, 112, 105, 110, 103, 32, 101, 114, 114,
            111, 114,
        ];
        assert_eq!(exp, &zerr.proto.as_bytes()[..20]);
        // find an offset
        let exp: &[u8] = &[
            250, 255, 255, 255, 15, 13, 119, 114, 97, 112, 112, 101, 100, 32, 101, 114, 114, 111,
            114,
        ];
        for idx in 0..zerr.proto.as_bytes().len() - exp.len() {
            if exp == &zerr.proto.as_bytes()[idx..idx + exp.len()] {
                return;
            }
        }
        let got: &[u8] = zerr.proto.as_bytes();
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
            ZError::value::<message<TestError>>(TestError::DefaultError(s))
        }

        fn case1(s: String) -> ZError<TestError> {
            ZError::value::<message<TestError>>(TestError::Case1(s))
        }

        fn case2(s: String) -> ZError<TestError> {
            ZError::value::<message<TestError>>(TestError::Case2(s))
        }
    }

    #[test]
    fn message() {
        let _ = TestError::default_error("default".to_string());
        let _ = TestError::case1("1".to_string());
        let _ = TestError::case2("2".to_string());
    }
}
