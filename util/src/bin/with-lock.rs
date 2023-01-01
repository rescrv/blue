use std::env::args;
use std::process::Command;

use util::lockfile::Lockfile;

fn main() {
    let args: Vec<String> = args().collect();
    // One arg for with-lock, one arg for lockfile.
    if args.len() < 2 {
        std::process::exit(-1);
    }
    let _lockfile = Lockfile::wait(args[1].clone());
    // Another arg for child.
    if args.len() < 3 {
        std::process::exit(-1);
    }
    // Return child exit status.
    let mut child = Command::new(args[2].clone())
        .args(&args[3..])
        .spawn().expect("exec failed");
    let exit_status = child.wait().expect("wait failed");
    std::process::exit(exit_status.code().unwrap_or(-1));
}
