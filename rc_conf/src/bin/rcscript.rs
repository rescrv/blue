//! ```
//! USAGE: RCVAR_ARGV0=${RCVAR_ARGV0} rcscript [--arguments-to-pass]
//! ```

use std::fs::read_to_string;

use utf8path::Path;

use rc_conf::RcScript;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    if args.len() < 2 {
        eprintln!("invoke with the name of the rcscript to run");
        std::process::exit(127);
    }

    let rc_contents = match read_to_string(&args[1]) {
        Ok(contents) => contents,
        Err(err) => {
            eprintln!("failed to read rcscript file {}: {err}", args[1]);
            std::process::exit(128);
        }
    };
    let mut rc_script = match RcScript::parse(&Path::from(args[1].clone()), &rc_contents) {
        Ok(script) => script,
        Err(err) => {
            eprintln!("failed to parse rcscript: {err}");
            std::process::exit(129);
        }
    };
    if let Ok(argv0) = std::env::var("RCVAR_ARGV0") {
        rc_script.set_name(argv0);
    }
    if let Err(err) = rc_script.invoke(&args[2..]) {
        eprintln!("error: {err:?}");
        std::process::exit(130);
    }
}
