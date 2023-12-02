#![doc = include_str!("../README.md")]

pub mod fmt;
pub mod fnmatch;
pub mod lockfile;
pub mod stopwatch;
pub mod time;

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
