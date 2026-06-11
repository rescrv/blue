#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::fd::AsRawFd;

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenMode {
    Read,
    Write,
}

#[cfg(not(unix))]
pub fn make_stdio(
    subs: &HashMap<char, String>,
    close: bool,
    file: Option<String>,
    mode: OpenMode,
) -> std::process::Stdio {
    if close {
        std::process::Stdio::null()
    } else if let Some(file) = file {
        std::process::Stdio::from(open_redirect_file(subs, file, mode))
    } else {
        std::process::Stdio::inherit()
    }
}

#[cfg(unix)]
pub fn close_or_dup2(
    subs: &HashMap<char, String>,
    close: bool,
    file: Option<String>,
    fd: libc::c_int,
    mode: OpenMode,
) {
    // take care of stdin
    if close {
        unsafe {
            // NOTE(rescrv):  On Linux, valid file descriptors cannot fail.  I don't think it's
            // worth failing over this, but maybe revisit and add error-handling.
            libc::close(fd);
        }
    } else if let Some(file) = file {
        let file = open_redirect_file(subs, file, mode);
        unsafe {
            if libc::dup2(file.as_raw_fd(), fd) < 0 {
                panic!("could not dup2: {:?}", std::io::Error::last_os_error());
            }
        }
    }
}

fn open_redirect_file(subs: &HashMap<char, String>, file: String, mode: OpenMode) -> std::fs::File {
    let Some(file) = pct_substitution(subs, &file) else {
        panic!("could not %-substitute {file}");
    };
    if !PathBuf::from(&file)
        .parent()
        .unwrap_or(Path::new(".."))
        .exists()
    {
        panic!("could not open {file}: containing directory does not exist");
    }
    let mut opts = OpenOptions::new();
    match mode {
        OpenMode::Read => opts.read(true),
        OpenMode::Write => opts.write(true).truncate(true).create(true),
    };
    opts.open(file).expect("file should open")
}

pub fn pct_substitution(subs: &HashMap<char, String>, input: &str) -> Option<String> {
    let mut prev = ' ';
    let mut output = String::with_capacity(input.len());
    for c in input.chars() {
        if prev == '%' {
            if c == '%' {
                output.push('%')
            } else if let Some(sub) = subs.get(&c) {
                output += sub;
            } else {
                return None;
            }
        } else if c != '%' {
            output.push(c);
        }
        prev = c;
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct() {
        let subs = HashMap::from([('p', "PID".to_string()), ('s', "TIME".to_string())]);
        assert_eq!(
            Some("foobar".to_string()),
            pct_substitution(&subs, "foobar")
        );
        assert_eq!(
            Some("foobar.PID.TIME".to_string()),
            pct_substitution(&subs, "foobar.%p.%s")
        );
        assert_eq!(None, pct_substitution(&subs, "foobar.%p.%t"));
    }
}
