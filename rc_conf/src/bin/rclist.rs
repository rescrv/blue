//! ```
//! USAGE: rclist <rc_d_path>
//! ```

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mut first = true;

    for path in args[1..].iter() {
        if !first {
            println!();
        }
        first = false;
        println!("PATH={path}");
        for (service, status) in
            rc_conf::load_services(path).expect("examine should always succeed")
        {
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
}
