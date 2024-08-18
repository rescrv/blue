use std::collections::HashMap;
use std::fs::OpenOptions;
use std::os::unix::process::CommandExt;
use std::time::SystemTime;

use arrrg::CommandLine;

use stdioredirect::close_or_dup2;

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct Options {
    #[arrrg(flag, "Close stdin.  Mutually exclusive with --stdin.")]
    close_stdin: bool,
    #[arrrg(flag, "Close stdout.  Mutually exclusive with --stdout.")]
    close_stdout: bool,
    #[arrrg(flag, "Close stderr.  Mutually exclusive with --stderr.")]
    close_stderr: bool,
    #[arrrg(optional, "Redirect stdin to this file in O_RDONLY mode.")]
    stdin: Option<String>,
    #[arrrg(
        optional,
        "Redirect stdout to this file in O_WRONLY mode, truncating and creating as necessary."
    )]
    stdout: Option<String>,
    #[arrrg(
        optional,
        "Redirect stderr to this file in O_WRONLY mode, truncating and creating as necessary."
    )]
    stderr: Option<String>,
}

fn main() {
    // option parsing
    let (options, free) =
        Options::from_command_line("USAGE: stdioredirect [--close-$stream|--$stream /file.txt] -- command [args]");
    if options.close_stdin && options.stdin.is_some() {
        eprintln!("mutually exclusive options --close-stdin and --stdin specified");
        std::process::exit(255);
    }
    if options.close_stdout && options.stdout.is_some() {
        eprintln!("mutually exclusive options --close-stdout and --stdout specified");
        std::process::exit(254);
    }
    if options.close_stderr && options.stderr.is_some() {
        eprintln!("mutually exclusive options --close-stderr and --stderr specified");
        std::process::exit(254);
    }
    // HashMap of chars to %-notation.
    let epoch_now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time should be ahead of epoch");
    let subs = HashMap::from([
        ('p', format!("{}", std::process::id())),
        ('s', format!("{}", epoch_now.as_secs())),
    ]);
    // take care of stdin
    let mut opts = OpenOptions::new();
    opts.read(true);
    close_or_dup2(
        &subs,
        options.close_stdin,
        options.stdin,
        libc::STDIN_FILENO,
        opts,
    );
    // take care of stdout
    let mut opts = OpenOptions::new();
    opts.write(true).truncate(true).create(true);
    close_or_dup2(
        &subs,
        options.close_stdout,
        options.stdout,
        libc::STDOUT_FILENO,
        opts,
    );
    // take care of stderr
    let mut opts = OpenOptions::new();
    opts.write(true).truncate(true).create(true);
    close_or_dup2(
        &subs,
        options.close_stderr,
        options.stderr,
        libc::STDERR_FILENO,
        opts,
    );
    // now exec
    panic!(
        "{:?}",
        std::process::Command::new(&free[0]).args(&free[1..]).exec()
    );
}
