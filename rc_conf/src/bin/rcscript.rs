use std::fs::read_to_string;

use utf8path::Path;

use rc_conf::RcScript;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    if args.len() < 2 {
        eprintln!("invoke with the name of the rcscript to run");
        std::process::exit(129);
    }

    let rc_contents = read_to_string(&args[1]).expect("rcscript should read to string");
    let rc_script = RcScript::parse(&Path::from(args[1].clone()), &rc_contents).expect("rcscript should parse");

    if let Err(err) = rc_script.invoke(&args[2..]) {
        eprintln!("error: {err:?}");
        std::process::exit(130);
    }
}
