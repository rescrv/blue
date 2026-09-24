//! ```
//! USAGE: rcwhy [OPTIONS] <service> [VAR...]
//! ```
//!
//! Explain how `service` resolves each VAR:  every name consulted in lookup order, the file and
//! line of each assignment (including the ones it overrides), which candidate won, what it expands
//! to, and how each variable referenced by the winner resolves.  With no VARs, explains `_ENABLED`
//! and every variable the service's rc.d stub reads.

use arrrg::CommandLine;

use rc_conf::{Provenance, RcConf};

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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rc_conf_path: "rc.conf".to_string(),
            rc_d_path: "rc.d".to_string(),
        }
    }
}

fn main() {
    let (options, argv) =
        Options::from_command_line_relaxed("USAGE: rcwhy [OPTIONS] <service> [VAR...]");
    if argv.is_empty() {
        eprintln!("expected service name to be provided");
        std::process::exit(129);
    }
    let service = argv[0].as_str();
    let rc_conf = RcConf::parse(&options.rc_conf_path).unwrap_or_else(|e| {
        eprintln!("failed to parse rc_conf: {e}");
        std::process::exit(133);
    });
    let provenance = Provenance::load(&options.rc_conf_path).unwrap_or_else(|e| {
        eprintln!("failed to load rc_conf provenance: {e}");
        std::process::exit(133);
    });
    let vars: Vec<String> = if argv.len() > 1 {
        argv[1..].to_vec()
    } else {
        print!("{}", rc_conf.why_switch(&provenance, service).render());
        let rc_d = rc_conf::load_services(&options.rc_d_path).unwrap_or_else(|e| {
            eprintln!("failed to load services: {e}");
            std::process::exit(134);
        });
        let target = rc_conf.resolve_alias(service);
        let path = match rc_d.get(target) {
            Some(Ok(path)) => path,
            Some(Err(err)) => {
                eprintln!("rc.d stub for {target} is unusable: {err}");
                std::process::exit(131);
            }
            None => {
                eprintln!("no rc.d stub for {target}; name variables explicitly to explain them");
                std::process::exit(130);
            }
        };
        let prefix = rc_conf::var_prefix_from_service(service);
        let keys = rc_conf.stub_rcvars(service, path).unwrap_or_else(|e| {
            eprintln!("failed to list rcvars: {e}");
            std::process::exit(135);
        });
        let mut vars = keys
            .iter()
            .filter_map(|k| k.strip_prefix(&prefix).map(String::from))
            .collect::<Vec<_>>();
        vars.sort();
        vars.dedup();
        vars
    };
    let mut failed = false;
    let mut first = argv.len() > 1;
    for var in vars {
        let why = rc_conf.why(&provenance, service, &var);
        failed |= why.expanded.is_err();
        if !first {
            println!();
        }
        first = false;
        print!("{}", why.render());
    }
    if failed {
        std::process::exit(1);
    }
}
