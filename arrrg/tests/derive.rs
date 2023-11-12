extern crate arrrg;
#[macro_use]
extern crate arrrg_derive;

use arrrg::CommandLine;

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct StringNumberOptions {
    #[arrrg(flag, "this toggles false->true only")]
    some_flag: bool,
    #[arrrg(required, "help text", "SYMBOL")]
    some_string: String,
    #[arrrg(optional, "optional u64", "SYMBOL")]
    some_u64: Option<u64>,
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct ExampleCommandLine {
    #[arrrg(flag, "this toggles false->true only")]
    top_flag: bool,
    #[arrrg(required, "a string at top level", "SYMBOL")]
    top_string: String,
    #[arrrg(optional, "an optional u64 at top", "SYMBOL")]
    top_u64: Option<u64>,
    #[arrrg(nested)]
    sno: StringNumberOptions,
}

fn test_helper(exp: &[&str], prefix: Option<&str>, example: ExampleCommandLine) {
    let mut opts = getopts::Options::new();
    example.add_opts(prefix, &mut opts);
    let matches = opts.parse(exp).expect("must parse args");
    let mut got = ExampleCommandLine {
        top_flag: false,
        top_string: String::default(),
        top_u64: None,
        sno: StringNumberOptions {
            some_flag: false,
            some_string: String::default(),
            some_u64: None,
        },
    };
    got.matches(prefix, &matches);
    assert_eq!(example, got);
    let got = example.canonical_command_line(prefix);
    let got: &[String] = &got;
    assert_eq!(exp, got);
}

#[test]
#[should_panic]
fn empty() {
    test_helper(
        &[],
        None,
        ExampleCommandLine {
            top_flag: false,
            top_string: String::new(),
            top_u64: None,
            sno: StringNumberOptions {
                some_flag: false,
                some_string: String::new(),
                some_u64: None,
            },
        },
    )
}

#[test]
fn req_args() {
    test_helper(
        &["--top-string", "xyz", "--sno-some-string", "abc"],
        None,
        ExampleCommandLine {
            top_flag: false,
            top_string: "xyz".to_owned(),
            top_u64: None,
            sno: StringNumberOptions {
                some_flag: false,
                some_string: "abc".to_owned(),
                some_u64: None,
            },
        },
    )
}

#[test]
fn all_args() {
    test_helper(
        &[
            "--top-flag",
            "--top-string",
            "xyz",
            "--top-u64",
            "456",
            "--sno-some-flag",
            "--sno-some-string",
            "abc",
            "--sno-some-u64",
            "123",
        ],
        None,
        ExampleCommandLine {
            top_flag: true,
            top_string: "xyz".to_owned(),
            top_u64: Some(456),
            sno: StringNumberOptions {
                some_flag: true,
                some_string: "abc".to_owned(),
                some_u64: Some(123),
            },
        },
    )
}
