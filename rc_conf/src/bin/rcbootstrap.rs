//! ```
//! USAGE: rcbootstrap --rc-conf-path RC_CONF_PATH <output>
//!
//! It will output an appropriate value for --rc-d-path and exit 0 on success, or proxy the error
//! code of cargo on failure.
//! ```

use arrrg::CommandLine;

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {
    #[arrrg(optional, "A colon-separated PATH-like list of rc.conf files to be loaded in order.  Later files override.")]
    rc_conf_path: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rc_conf_path: "rc.conf".to_string(),
        }
    }
}

fn main() {
    let (options, argv) = Options::from_command_line_relaxed(
        "USAGE: rcbootstrap --rc-conf-path RC_CONF_PATH <output>",
    );
    let argv = argv.iter().map(|a| a.as_str()).collect::<Vec<_>>();
    if argv.len() != 1 {
        eprintln!("expected solely the output path to be provided");
        std::process::exit(129);
    }
    match rc_conf::bootstrap(&options.rc_conf_path, argv[0]) {
        Ok(rc_d_path) => {
            println!("{rc_d_path}");
        }
        Err(err) => {
            eprintln!("{err:#?}");
        }
    }
}
