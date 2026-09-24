//! ```
//! USAGE: rclint [OPTIONS]
//! ```
//!
//! Lint an rc_conf path against an rc.d path.  Reports invalid `_ENABLED` values (which rc_conf
//! otherwise silently treats as NO), enabled services without a stub, stubs that fail `rcvar`,
//! values that fail to expand, variables no service reads, duplicate assignments within a file,
//! stub variables with no value, and stubs nothing enables.
//!
//! Exits 1 if there are errors (or warnings, with --strict), 2 if the configuration fails to parse.

use arrrg::CommandLine;

use rc_conf::{LintOptions, Severity};

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
        optional,
        "Comma-separated variable names or suffixes read by other tools (e.g. IMAGE,CONTAINERFILE)."
    )]
    allow: String,
    #[arrrg(flag, "Exit non-zero on warnings as well as errors.")]
    strict: bool,
    #[arrrg(flag, "Suppress notes.")]
    quiet: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
            allow: String::new(),
            strict: false,
            quiet: false,
        }
    }
}

fn main() {
    let (options, argv) = Options::from_command_line("USAGE: rclint [OPTIONS]");
    if !argv.is_empty() {
        eprintln!("rclint takes no positional arguments");
        std::process::exit(129);
    }
    let lint_options = LintOptions {
        allow: options
            .allow
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
    };
    let findings = match rc_conf::lint(&options.rc_conf_path, &options.rc_d_path, &lint_options) {
        Ok(findings) => findings,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };
    let mut errors = 0;
    let mut warnings = 0;
    for finding in findings.iter() {
        match finding.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Note if options.quiet => continue,
            Severity::Note => {}
        }
        println!("{finding}");
    }
    if errors > 0 || (options.strict && warnings > 0) {
        std::process::exit(1);
    }
}
