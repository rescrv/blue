#![doc = include_str!("../README.md")]

/// Formatting utils.
pub mod fmt;

/// Pattern matching utils.
pub mod fnmatch;

/// UNIX-like lockfile utils
pub mod lockfile;

/// A stopwatch.
pub mod stopwatch;

/// Utilities for dealing with time.
pub mod time;

/// A macro that invokes `$name` for each test name => expression provided.
#[macro_export]
macro_rules! test_per_file {
    ($name:ident { $($func:ident => $s:expr,)+ }) => {
        $(
            #[test]
            fn $func() {
                $name(file!(), line!(), $s);
            }
        )+
    }
}

#[cfg(test)]
mod tests {
    fn do_test(_file: &str, _line: u32, s: &str) {
        println!("FINDME: s={}", s);
    }

    test_per_file! {
        do_test {
            a => "a",
            b => "b",
        }
    }
}
