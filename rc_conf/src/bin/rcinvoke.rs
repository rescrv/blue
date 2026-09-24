//! ```
//! USAGE: rcinvoke [OPTIONS] [--dry-run] <service> [ARGS]
//! ```

use arrrg::CommandLine;

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {
    #[arrrg(
        optional,
        "A colon-separated PATH-like list of rc.conf files to be loaded in order.  Later files override."
    )]
    rc_conf_path: String,
    #[arrrg(
        optional,
        "A colon-separated PATH-like list of rc.d directories to be scanned in order.  Earlier files short-circuit."
    )]
    rc_d_path: String,
    #[arrrg(
        flag,
        "Print the command rcinvoke would exec (as an env(1) invocation) instead of running it."
    )]
    dry_run: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
            dry_run: false,
        }
    }
}

fn main() {
    let (options, argv) =
        Options::from_command_line_relaxed("USAGE: rcinvoke [OPTIONS] <service> [ARGS]");
    let argv = argv.iter().map(|a| a.as_str()).collect::<Vec<_>>();
    if argv.is_empty() {
        eprintln!("expected service name to be provided");
        std::process::exit(129);
    }
    if options.dry_run {
        let mut cmd = vec!["run"];
        cmd.extend(argv[1..].iter());
        match rc_conf::plan_invoke(&options.rc_conf_path, &options.rc_d_path, argv[0], &cmd) {
            Ok(invocation) => {
                println!("{}", invocation.to_shell());
                return;
            }
            Err((code, msg)) => {
                eprintln!("{msg}");
                std::process::exit(code);
            }
        }
    }
    rc_conf::invoke(
        &options.rc_conf_path,
        &options.rc_d_path,
        argv[0],
        &argv[1..],
    );
}
