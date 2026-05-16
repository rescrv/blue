//! ```
//! USAGE: rclist <rc_d_path>
//! ```

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mut first = true;
    let mut failed = false;

    for path in args[1..].iter() {
        if !first {
            println!();
        }
        first = false;
        println!("PATH={path}");
        match rc_conf::load_services(path) {
            Ok(services) => {
                for (service, status) in services {
                    match status {
                        Ok(path) => {
                            println!("{service}\t{path:?}");
                        }
                        Err(why) => {
                            println!("{service} encountered error: {why}");
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("failed to load services from {path}: {err}");
                failed = true;
                continue;
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
}
