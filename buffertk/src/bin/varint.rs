//! For each u64 provided as an argument on the command line, print the varint representation as
//! bytes.

use buffertk::Packable;

fn main() {
    for argument in std::env::args().skip(1) {
        let x = match argument.parse::<u64>() {
            Ok(x) => x,
            Err(e) => {
                eprintln!("don't know how to parse {argument}: {e}");
                continue;
            }
        };
        let v: buffertk::v64 = x.into();
        let mut pirate = [0u8; 10];
        let pirate: &mut [u8] = &mut pirate;
        let pa = buffertk::stack_pack(v);
        pa.into_slice(pirate);
        println!("{:?}", &pirate[..pa.pack_sz()]);
    }
}
