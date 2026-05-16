//! ```
//! USAGE: rcexamine <rc_conf_path>
//! ```

use rc_conf::RcConf;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mut failed = false;

    for path in args[1..].iter() {
        match RcConf::examine(path) {
            Ok(conf) => println!("{conf}"),
            Err(err) => {
                eprintln!("failed to examine rc_conf path {path}: {err}");
                failed = true;
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
}
