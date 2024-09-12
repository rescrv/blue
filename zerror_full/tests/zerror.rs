use biometrics::Counter;
use prototk_derive::Message;
use zerror_full::zerror;

mod somemod {
    #[derive(Debug, Default, prototk_derive::Message)]
    pub struct Error;

    impl std::fmt::Display for Error {
        fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
            write!(fmt, "<ERROR>")
        }
    }

    impl std::error::Error for Error {}
}

mod othermod {
    #[derive(Debug, Default, prototk_derive::Message)]
    pub struct Error;

    impl std::fmt::Display for Error {
        fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
            write!(fmt, "<ERROR>")
        }
    }

    impl std::error::Error for Error {}
}

static LOGIC_ERROR: Counter = Counter::new("tests.LOGIC_ERROR");
static OTHER_MOD: Counter = Counter::new("tests.OTHER_MOD");

zerror! {
    #[derive(Message)]
    TestError {
        #[prototk(1, message)]
        Success as success {},
        #[prototk(2, message)]
        LogicError as logic counter LOGIC_ERROR {
            #[prototk(2, string)]
            what: String,
            #[prototk(3, uint64)]
            number: u64,
        },
        #[prototk(3, message)]
        FoobaError as fooba {
            #[prototk(2, string)]
            what: String,
            #[prototk(3, uint64)]
            number: u64,
        },
        #[prototk(4, message)]
        SomeMod from
        #[prototk(2, message)]
        somemod::Error
        as somemod,
        #[prototk(5, message)]
        OtherMod from
        #[prototk(2, message)]
        othermod::Error
        as othermod counter OTHER_MOD,
    }
}

impl Default for TestError {
    fn default() -> Self {
        todo!();
    }
}

#[test]
fn testit() {
    let _te = TestError::success();
    let _te = TestError::logic("what", 42u64);
    let _te = TestError::fooba("what", 42u64);
    let _te = TestError::from(somemod::Error);
}
