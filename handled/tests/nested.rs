use biometrics::Counter;

use handled::{handled, HasCore, ResultExt};

static SYSTEM_ERROR: Counter = Counter::new("system_error");
static ERROR1: Counter = Counter::new("error1");

handled! {
    #[derive(Eq, PartialEq, Hash)]
    Error1 {
        System as system @ SYSTEM_ERROR {
            core,
            what: String,
        },
    }
}

handled! {
    #[derive(Eq, PartialEq, Hash)]
    Error2 {
        Error1 as error1 @ ERROR1 {
            core,
            what: Error1,
        },
    }
}

impl From<Error1> for Error2 {
    fn from(e: Error1) -> Self {
        Error2::error1(e)
    }
}

fn ex1() -> Result<(), Error1> {
    Err(Error1::system("My Error1 Message".to_string()))
}

fn ex2() -> Result<(), Error2> {
    ex1().without_backtrace()?;
    Ok(())
}

#[test]
fn error() {
    let mut err1 = Error1::system("My Error1 Message".to_string());
    err1.core_mut().without_backtrace();
    let mut err2 = Error2::error1(err1);
    err2.core_mut().without_backtrace();
    assert_eq!(Err(err2), ex2().without_backtrace());
}
