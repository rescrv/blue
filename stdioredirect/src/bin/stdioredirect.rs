use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;

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
    if options.close_stdin {
        unsafe {
            libc::close(libc::STDIN_FILENO);
        }
    } else if let Some(stdin) = options.stdin {
        let stdin = OpenOptions::new().read(true).open(stdin).unwrap();
        unsafe {
            if libc::dup2(stdin.as_raw_fd(), libc::STDIN_FILENO) < 0 {
                panic!("{:?}", std::io::Error::last_os_error());
            }
        }
    }
    // take care of stdout
    if options.close_stdout {
        unsafe {
            libc::close(libc::STDOUT_FILENO);
        }
    } else if let Some(stdout) = options.stdout {
        let stdout = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(stdout)
            .unwrap();
        unsafe {
            if libc::dup2(stdout.as_raw_fd(), libc::STDOUT_FILENO) < 0 {
                panic!("{:?}", std::io::Error::last_os_error());
            }
        }
    }
    // take care of stderr
    if options.close_stderr {
        unsafe {
            libc::close(libc::STDERR_FILENO);
        }
    } else if let Some(stderr) = options.stderr {
        let stderr = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(stderr)
            .unwrap();
        unsafe {
            if libc::dup2(stderr.as_raw_fd(), libc::STDERR_FILENO) < 0 {
                panic!("{:?}", std::io::Error::last_os_error());
            }
        }
    }
    // now exec
    panic!(
        "{:?}",
        std::process::Command::new(&free[0]).args(&free[1..]).exec()
    );
}
