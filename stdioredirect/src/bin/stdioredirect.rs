use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;

use std::path::{Path, PathBuf};

use arrrg::CommandLine;

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

fn close_or_dup2(close: bool, file: Option<String>, fd: libc::c_int, opts: OpenOptions) {
    // take care of stdin
    if close {
        unsafe {
            // NOTE(rescrv):  On Linux, valid file descriptors cannot fail.  I don't think it's
            // worth failing over this, but maybe revisit and add error-handling.
            libc::close(fd);
        }
    } else if let Some(file) = file {
        if !PathBuf::from(&file)
            .parent()
            .unwrap_or(Path::new(".."))
            .exists()
        {
            panic!("could not open {file}: containing directory does not exist");
        }
        let file = opts.open(file).expect("file should open");
        unsafe {
            if libc::dup2(file.as_raw_fd(), fd) < 0 {
                panic!("could not dup2: {:?}", std::io::Error::last_os_error());
            }
        }
    }
}

fn main() {
    // option parsing
    let (options, free) =
        Options::from_command_line("USAGE: stdioredirect [--close-$stream|--$stream /file.txt]");
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
    // take care of stdin
    let mut opts = OpenOptions::new();
    opts.read(true);
    close_or_dup2(options.close_stdin, options.stdin, libc::STDIN_FILENO, opts);
    // take care of stdout
    let mut opts = OpenOptions::new();
    opts.write(true).truncate(true).create(true);
    close_or_dup2(
        options.close_stdout,
        options.stdout,
        libc::STDOUT_FILENO,
        opts,
    );
    // take care of stderr
    let mut opts = OpenOptions::new();
    opts.write(true).truncate(true).create(true);
    close_or_dup2(
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
