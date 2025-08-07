use biometrics::Counter;

use handled::{handle, handled, no_quote_debug, ResultExt};

static SYSTEM_ERROR: Counter = Counter::new("system_error");

handled! {
    #[derive(Eq, PartialEq, Hash)]
    Error {
        System as system @ SYSTEM_ERROR {
            core,
            what: String,
        },
    }
}

#[test]
fn error() {
    let system = Error::system("My Message".to_string());
    assert!(matches!(system, Error::System { .. }));
}

#[test]
fn error_with_raw_info() {
    fn fail1(s: &str) -> Result<(), Error> {
        Err(Error::system(s.to_string())).without_backtrace()
    }
    fn fail2(s1: &str, s2: &str) -> Result<(), Error> {
        Err(Error::system(format!("{s1} {s2}"))).without_backtrace()
    }
    // Test with_info.
    let res = handle!(fail1, "noexist.txt");
    assert_eq!(
        Err(Error::system("noexist.txt".to_string()))
            .with_info("\"noexist.txt\"", "noexist.txt")
            .without_backtrace(),
        res
    );
    // Test with_info with multiple arguments.
    let res = handle!(fail2, "noexist.txt", "really");
    assert_eq!(
        Err(Error::system("noexist.txt really".to_string()))
            .with_info("\"noexist.txt\"", no_quote_debug("\"noexist.txt\""))
            .with_info("\"really\"", no_quote_debug("\"really\""))
            .without_backtrace(),
        res
    );
    // Test with_info with skip argument.
    let res = handle!(fail2, "noexist.txt", skip "really");
    assert_eq!(
        Err(Error::system("noexist.txt really".to_string()))
            .with_info("\"noexist.txt\"", "noexist.txt")
            .without_backtrace(),
        res
    );
    // Test with_info with named argument.
    let res = handle!(fail1, file = "noexist.txt");
    assert_eq!(
        Err(Error::system("noexist.txt".to_string()))
            .with_info("file", "noexist.txt")
            .without_backtrace(),
        res
    );
}
