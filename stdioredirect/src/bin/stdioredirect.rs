use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::time::SystemTime;

use arrrg::CommandLine;

use stdioredirect::OpenMode;
#[cfg(unix)]
use stdioredirect::close_or_dup2;
#[cfg(not(unix))]
use stdioredirect::make_stdio;

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
    let (options, free) = Options::from_command_line(
        "USAGE: stdioredirect [--close-$stream|--$stream /file.txt] -- command [args]",
    );
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
    if free.is_empty() {
        eprintln!("no command specified");
        std::process::exit(253);
    }
    // HashMap of chars to %-notation.
    let epoch_now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time should be ahead of epoch");
    let subs = HashMap::from([
        ('p', format!("{}", std::process::id())),
        ('s', format!("{}", epoch_now.as_secs())),
    ]);

    let mut cmd = std::process::Command::new(&free[0]);
    cmd.args(&free[1..]);

    #[cfg(unix)]
    {
        // take care of stdin
        close_or_dup2(
            &subs,
            options.close_stdin,
            options.stdin,
            libc::STDIN_FILENO,
            OpenMode::Read,
        );
        // take care of stdout
        close_or_dup2(
            &subs,
            options.close_stdout,
            options.stdout,
            libc::STDOUT_FILENO,
            OpenMode::Write,
        );
        // take care of stderr
        close_or_dup2(
            &subs,
            options.close_stderr,
            options.stderr,
            libc::STDERR_FILENO,
            OpenMode::Write,
        );
        // now exec
        panic!("{:?}", cmd.exec());
    }

    #[cfg(not(unix))]
    {
        let status = cmd
            .stdin(make_stdio(
                &subs,
                options.close_stdin,
                options.stdin,
                OpenMode::Read,
            ))
            .stdout(make_stdio(
                &subs,
                options.close_stdout,
                options.stdout,
                OpenMode::Write,
            ))
            .stderr(make_stdio(
                &subs,
                options.close_stderr,
                options.stderr,
                OpenMode::Write,
            ))
            .status()
            .expect("failed to execute process");
        std::process::exit(status.code().unwrap_or(1));
    }
}
