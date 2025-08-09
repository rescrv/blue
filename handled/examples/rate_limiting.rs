use handled::{Handle, HandleResult};

#[derive(Clone, Debug, Eq, PartialEq)]
struct RateLimit {
    wait_ms: u32,
    debug: String,
}

impl std::fmt::Display for RateLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Handle<RateLimit> for RateLimit {
    fn handle(&self) -> Option<Self> {
        Some(self.clone())
    }
}

#[derive(Clone, Debug)]
enum MyCustomError {
    Variant1(Type1),
    Variant2(Type2),
    Variant3(Error1),
}

impl std::fmt::Display for MyCustomError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for MyCustomError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Variant1(_) => None,
            Self::Variant2(_) => None,
            Self::Variant3(e) => Some(e),
        }
    }
}

impl Handle<RateLimit> for MyCustomError {
    fn handle(&self) -> Option<RateLimit> {
        match self {
            Self::Variant1(_) => None,
            Self::Variant2(x) => x.handle(),
            Self::Variant3(x) => x.handle(),
        }
    }
}

#[derive(Clone, Debug)]
enum Type1 {
    SomeErrorCode,
}

#[derive(Clone, Debug)]
struct Type2 {
    code: u16,
    text: String,
}

impl Handle<RateLimit> for Type2 {
    fn handle(&self) -> Option<RateLimit> {
        if self.code == 429 {
            Some(RateLimit {
                // NOTE(rescrv):  When copying, do not use this value.
                wait_ms: 42,
                debug: self.text.clone(),
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
struct Error1 {}

impl std::fmt::Display for Error1 {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error1 {}

impl Handle<RateLimit> for Error1 {
    fn handle(&self) -> Option<RateLimit> {
        Some(RateLimit {
            // NOTE(rescrv):  When copying, do not use this value.
            wait_ms: 84,
            debug: "Slow down!".to_string(),
        })
    }
}

fn main() {
    let err1 = MyCustomError::Variant1(Type1::SomeErrorCode);
    let err2 = MyCustomError::Variant2(Type2 {
        code: 429,
        text: "Slow down!".to_string(),
    });
    let err3 = MyCustomError::Variant3(Error1 {});
    let rate1: Option<RateLimit> = err1.handle();
    assert!(rate1.is_none());
    let rate2: Option<RateLimit> = err2.handle();
    assert_eq!(
        Some(RateLimit {
            wait_ms: 42,
            debug: "Slow down!".to_string()
        }),
        rate2
    );
    let rate3: Option<RateLimit> = err3.handle();
    assert_eq!(
        Some(RateLimit {
            wait_ms: 84,
            debug: "Slow down!".to_string()
        }),
        rate3
    );
    let result = Err::<(), _>(MyCustomError::Variant2(Type2 {
        code: 429,
        text: "Slow down!".to_string(),
    }));
    let result: Result<_, RateLimit> = result.handle_result();
    assert_eq!(
        RateLimit {
            wait_ms: 42,
            debug: "Slow down!".to_string()
        },
        result.unwrap_err()
    );
    let result1 = Err::<(), _>(MyCustomError::Variant1(Type1::SomeErrorCode));
    assert_eq!("Err(Variant1(SomeErrorCode))", format!("{result1:?}"));
    let result2: Result<_, RateLimit> = result1.handle_result();
    assert_eq!("Ok(Err(Variant1(SomeErrorCode)))", format!("{result2:?}"));
}
