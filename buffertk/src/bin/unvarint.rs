//! For each u64 provided as an argument on the command line, print the varint representation as
//! bytes.  Provide the bytes in base 10, one argument per byte.

use buffertk::Unpackable;

fn main() {
    let mut bytes = Vec::new();
    for argument in std::env::args().skip(1) {
        let x = match argument.parse::<u8>() {
            Ok(x) => x,
            Err(e) => {
                eprintln!("don't know how to parse {argument}: {e}");
                std::process::exit(1);
            }
        };
        bytes.push(x);
    }
    let v: u64 = buffertk::v64::unpack(&bytes).unwrap().0.into();
    println!("{v}");
}
