use guacamole::combinators::*;
use guacamole::Guacamole;
use statslicer::{benchmark, black_box, statslicer_main, Bencher, Parameter, Parameters};

use buffertk::{stack_pack, v64, Unpackable};

const BYTES: &[usize] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

#[derive(Debug, Default, Eq, PartialEq)]
struct VarintParameters {
    bytes: usize,
}

impl Parameters for VarintParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![("bytes", Parameter::Integer(self.bytes as u64))]
    }
}

/////////////////////////////////////////////// util ///////////////////////////////////////////////

fn generate_varints(params: &VarintParameters, guac: &mut Guacamole, size: usize) -> Vec<u8> {
    let mut buf = vec![];
    for _ in 0..size {
        let lower = (params.bytes - 1) << 7;
        let upper = (params.bytes << 7) - 1;
        let x = uniform(lower, upper)(guac);
        stack_pack(v64::from(x)).append_to_vec(&mut buf);
    }
    buf
}

//////////////////////////////////////////////// new ///////////////////////////////////////////////

fn bench_varint_unpack(params: &VarintParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let varints = generate_varints(params, &mut guac, b.size());
    fn unpack(_: &VarintParameters, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let x: v64;
            (x, bytes) = v64::unpack(bytes).unwrap();
            black_box(x);
        }
    }
    b.run(|| {
        unpack(params, &varints);
    });
}

benchmark! {
    name = varint_unpack;
    VarintParameters {
        bytes in BYTES,
    }
    bench_varint_unpack
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

statslicer_main! {
    varint_unpack
}
