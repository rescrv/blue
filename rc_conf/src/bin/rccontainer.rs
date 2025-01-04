//! ```
//! USAGE: rccontainer [OPTIONS] <container-name> <service> [ARGS]
//! ```

use arrrg::CommandLine;

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {
    #[arrrg(optional, "A colon-separated PATH-like list of rc.conf files to be loaded in order.  Later files override.")]
    rc_conf_path: String,
    #[arrrg(optional, "A colon-separated PATH-like list of rc.d directories to be scanned in order.  Earlier files short-circuit.")]
    rc_d_path: String,
    #[arrrg(optional, "The container runtime (docker or podman)")]
    runtime: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
            runtime: "docker".to_string(),
        }
    }
}

fn main() {
    let (options, argv) = Options::from_command_line_relaxed(
        "USAGE: rccontainer [OPTIONS] <container-name> <service> [ARGS]",
    );
    let argv = argv.iter().map(|a| a.as_str()).collect::<Vec<_>>();
    if argv.len() < 2 {
        eprintln!("expected container and service name to be provided");
        std::process::exit(129);
    }
    rc_conf::exec_container(
        &options.rc_conf_path,
        &options.rc_d_path,
        &options.runtime,
        argv[0],
        argv[1],
        &argv[2..],
    );
}
