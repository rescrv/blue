//! ```
//! USAGE: rcdebug <rc_conf_path>
//! ```

use rc_conf::RcConf;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    for path in args[1..].iter() {
        println!(
            "{:#?}",
            RcConf::parse(path).expect("parse should always succeed")
        );
    }
}
