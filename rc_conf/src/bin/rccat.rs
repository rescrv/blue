//! This is a simple utility to format a string from a configuration.
//!
//! It is intended for free-form text of playbooks, runbooks, or other documentation that belongs
//! alongside binaries or services.
//!
//! ```
//! USAGE: rccat run << EOF
//! # Nginx 500 Error Response Playbook ($ENVIRONMENT)
//!
//! * **Initial Response (0-5 minutes)**: Acknowledge error, notify team/stakeholders, and verify
//!   Nginx as error source.
//! * **Diagnostic Phase**: Review Nginx error logs, check configuration files, assess server
//!   resources, and inspect backend application logs (if applicable).
//! * **Resolution Phase**: Address identified cause, apply fix, restart Nginx if needed, and
//!   verify resolution.
//! * **Post-Incident**: Conduct root cause analysis, update documentation, and notify stakeholders
//!   of resolution.
//! * **Review Schedule**: Every 6 months or after significant infrastructure changes.
//! EOF
//! # Nginx 500 Error Response Playbook (production)
//!
//! * **Initial Response (0-5 minutes)**: Acknowledge error, notify team/stakeholders, and verify
//!   Nginx as error source.
//! * **Diagnostic Phase**: Review Nginx error logs, check configuration files, assess server
//!   resources, and inspect backend application logs (if applicable).
//! * **Resolution Phase**: Address identified cause, apply fix, restart Nginx if needed, and
//!   verify resolution.
//! * **Post-Incident**: Conduct root cause analysis, update documentation, and notify stakeholders
//!   of resolution.
//! * **Review Schedule**: Every 6 months or after significant infrastructure changes.
//! ```

use std::collections::HashMap;

use utf8path::Path;

fn main() {
    // Parse arguments.
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("USAGE: rccat file [describe|rcvar|run]");
        std::process::exit(1);
    }
    // Figure out the name of the service.
    let name = if let Ok(path) = std::env::var("RCVAR_ARGV0") {
        path.to_string()
    } else {
        rc_conf::name_from_path(&Path::new(&args[0]))
    };
    let prefix = rc_conf::var_prefix_from_service(&name);
    // Read stdin.
    let stdin = std::fs::read_to_string(&args[1]).unwrap();
    if args[2] == "describe" {
        println!("{}", stdin.trim_end());
    } else if args[2] == "rcvar" {
        let mut rcvar = shvar::rcvar(&stdin)
            .expect("shell variable substitution should be valid")
            .into_iter()
            .map(|v| format!("{prefix}{v}"))
            .collect::<Vec<_>>();
        rcvar.sort();
        println!("{}", rcvar.join("\n"));
    } else if args[2] == "run" {
        let evp = rc_conf::EnvironmentVariableProvider::new(Some(prefix.clone()));
        let meta = HashMap::from([("NAME".to_string(), prefix)]);
        for line in stdin.lines() {
            let out = shvar::expand_recursive(&(&meta, &evp), line)
                .expect("shell variable expansion should be valid");
            println!("{}", out.trim_end());
        }
    } else {
        eprintln!("USAGE: rccat [describe|rcvar|run]");
        std::process::exit(1);
    }
}
