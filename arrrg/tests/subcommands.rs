extern crate arrrg;
#[macro_use]
extern crate arrrg_derive;

use arrrg::CommandLine;

#[derive(Clone, Debug, Default, Eq, PartialEq, CommandLine)]
struct RootOptions {
    #[arrrg(optional, "root argument one")]
    arg1: String,
    #[arrrg(flag, "root argument two")]
    arg2: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, CommandLine)]
struct Subcommand1Options {
    #[arrrg(optional, "subcommand one argument one")]
    sc1_arg1: String,
    #[arrrg(flag, "subcommand one argument two")]
    sc1_arg2: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, CommandLine)]
struct Subcommand2Options {
    #[arrrg(required, "subcommand two argument one")]
    sc2_arg1: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, CommandLine)]
struct OtherOptions {
    #[arrrg(flag, "unused option")]
    other: bool,
}

#[test]
fn recursive_subcommands_parse_each_layer() {
    let args = [
        "--arg1",
        "root",
        "--arg2",
        "subcommand1",
        "--sc1-arg1",
        "one",
        "--sc1-arg2",
        "subcommand2",
        "--sc2-arg1",
        "two",
        "positional",
    ];
    let (root, free) = RootOptions::from_arguments("Usage: binary [OPTIONS] <command>", &args);

    let result = arrrg::dispatch_subcommands!(free, {
        "subcommand1" => Subcommand1Options as sc1, sc1_free => {
            arrrg::dispatch_subcommands!(sc1_free, {
                "subcommand2" => Subcommand2Options as sc2, sc2_free => {
                    Ok((root, sc1, sc2, sc2_free))
                },
                "other" => OtherOptions as other, other_free => {
                    Ok((root, Subcommand1Options::default(), Subcommand2Options {
                        sc2_arg1: format!("{:?}:{:?}", other, other_free),
                    }, Vec::new()))
                },
            })
        },
        "other" => OtherOptions as other, other_free => {
            Ok((root, Subcommand1Options::default(), Subcommand2Options {
                sc2_arg1: format!("{:?}:{:?}", other, other_free),
            }, Vec::new()))
        },
    });

    assert_eq!(
        Ok((
            RootOptions {
                arg1: "root".to_string(),
                arg2: true,
            },
            Subcommand1Options {
                sc1_arg1: "one".to_string(),
                sc1_arg2: true,
            },
            Subcommand2Options {
                sc2_arg1: "two".to_string(),
            },
            vec!["positional".to_string()],
        )),
        result,
    );
}

#[test]
fn split_subcommand_reports_missing() {
    assert_eq!(
        Err(arrrg::SubcommandError::missing(vec![
            "one".to_string(),
            "two".to_string()
        ])),
        arrrg::split_subcommand(vec![], &["one", "two"]),
    );
}

#[test]
fn split_subcommand_reports_unknown() {
    assert_eq!(
        Err(arrrg::SubcommandError::unknown(
            "wat".to_string(),
            vec!["one".to_string(), "two".to_string()],
        )),
        arrrg::split_subcommand(vec!["wat".to_string()], &["one", "two"]),
    );
}
