//! ```
//! USAGE: rcdebug <rc_conf_path>
//! ```

use rc_conf::RcConf;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mut failed = false;

    for path in args[1..].iter() {
        match RcConf::parse(path) {
            Ok(conf) => println!("{conf:#?}"),
            Err(err) => {
                eprintln!("failed to parse rc_conf path {path}: {err}");
                failed = true;
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
}
