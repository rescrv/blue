extern crate arrrg;
#[macro_use]
extern crate arrrg_derive;

use arrrg::CommandLine;

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct DifferentKindsOfOptions {
    #[arrrg(optional, "optional u64", "SYMBOL")]
    option_u64: Option<u64>,
    #[arrrg(optional, "optional u64", "SYMBOL")]
    just_u64: u64,
}

fn test_helper(exp: &[&str], prefix: Option<&str>, example: DifferentKindsOfOptions) {
    let mut opts = getopts::Options::new();
    example.add_opts(prefix, &mut opts);
    let matches = opts.parse(exp).expect("must parse args");
    let mut got = DifferentKindsOfOptions::default();
    got.matches(prefix, &matches);
    assert_eq!(example, got);
    let got = example.canonical_command_line(prefix);
    let got: &[String] = &got;
    assert_eq!(exp, got);
}

#[test]
fn empty() {
    test_helper(
        &[],
        None,
        DifferentKindsOfOptions {
            option_u64: None,
            just_u64: 0,
        },
    );
}

#[test]
fn option_u64() {
    test_helper(
        &["--option-u64", "42"],
        None,
        DifferentKindsOfOptions {
            option_u64: Some(42),
            just_u64: 0,
        },
    );
}

#[test]
fn just_u64() {
    test_helper(
        &["--just-u64", "42"],
        None,
        DifferentKindsOfOptions {
            option_u64: None,
            just_u64: 42,
        },
    );
}
