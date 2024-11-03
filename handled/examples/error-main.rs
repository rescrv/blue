use biometrics::Counter;

use handled::handled;

static LOGIC_ERROR: Counter = Counter::new("logic_error");
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
        Logic as logic @ LOGIC_ERROR {
            core,
            what: String,
        },
        Error1 as error1 @ ERROR1 {
            core,
            what: Error1,
        },
    }
}

fn main() -> Result<(), Error2> {
    Err::<(), _>(Error2::error1(Error1::system(
        "My Error1 Message".to_string(),
    )))
}
