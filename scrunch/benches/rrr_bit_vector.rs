use guacamole::combinators::*;
use guacamole::Guacamole;
use statslicer::{benchmark, black_box, statslicer_main, Bencher, Parameter, Parameters};

use scrunch::bit_vector::rrr::BitVector;
use scrunch::bit_vector::BitVector as BitVectorTrait;
use scrunch::builder::Builder;

const VECTOR_LENGTH: &[usize] = &[
    256, 1024, 4096, 16384, 65536, 262144, 1048576, 4194304, 16777216, 67108864, 268435456,
];

const BIT_PROBABILITY: &[f32] = &[0.0, 0.125, 0.25, 0.5, 0.75, 0.875, 1.0];

#[derive(Debug, Default)]
struct RrrBitVectorParameters {
    vector_length: usize,
    probability: f32,
}

impl Parameters for RrrBitVectorParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![
            (
                "vector_length",
                Parameter::Integer(self.vector_length as u64),
            ),
            ("probability", Parameter::Float(self.probability as f64)),
        ]
    }
}

/////////////////////////////////////////////// util ///////////////////////////////////////////////

fn generate_bit_vector(params: &RrrBitVectorParameters, guac: &mut Guacamole) -> Vec<bool> {
    let mut bits = vec![];
    while bits.len() < params.vector_length {
        bits.push(prob(params.probability)(guac));
    }
    bits
}

//////////////////////////////////////////////// new ///////////////////////////////////////////////

fn bench_bit_vector_new(params: &RrrBitVectorParameters, b: &mut Bencher) {
    let mut vectors = vec![];
    let mut guac = Guacamole::new(b.seed());
    for _ in 0..b.size() {
        vectors.push(generate_bit_vector(params, &mut guac));
    }
    fn construct(bits: &[bool]) -> Vec<u8> {
        let mut buf = vec![];
        let mut builder = Builder::new(&mut buf);
        BitVector::construct(&bits, &mut builder).unwrap();
        drop(builder);
        black_box(buf)
    }
    b.run(|| {
        for vector in vectors.into_iter() {
            construct(black_box(&vector));
        }
    });
}

benchmark! {
    name = rrr_bit_vector_new;
    RrrBitVectorParameters {
        vector_length in VECTOR_LENGTH,
        probability in BIT_PROBABILITY,
    }
    bench_bit_vector_new
}

////////////////////////////////////////////// access //////////////////////////////////////////////

fn bench_bit_vector_access(params: &RrrBitVectorParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let mut buf = vec![];
    let mut builder = Builder::new(&mut buf);
    let vector = generate_bit_vector(params, &mut guac);
    BitVector::construct(&vector, &mut builder).unwrap();
    drop(builder);
    let vector = BitVector::parse(&buf).unwrap().0;
    let mut accesses = Vec::with_capacity(b.size());
    for _ in 0..b.size() {
        accesses.push(range_to(vector.len())(&mut guac));
    }
    fn access(bv: &BitVector, accesses: &[usize]) {
        for access in accesses {
            black_box(bv).access(black_box(*access));
        }
    }
    b.run(|| {
        access(&vector, &accesses);
    });
}

benchmark! {
    name = rrr_bit_vector_access;
    RrrBitVectorParameters {
        vector_length in VECTOR_LENGTH,
        probability in BIT_PROBABILITY,
    }
    bench_bit_vector_access
}

/////////////////////////////////////////////// rank ///////////////////////////////////////////////

fn bench_bit_vector_rank(params: &RrrBitVectorParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let mut buf = vec![];
    let mut builder = Builder::new(&mut buf);
    let vector = generate_bit_vector(params, &mut guac);
    BitVector::construct(&vector, &mut builder).unwrap();
    drop(builder);
    let vector = BitVector::parse(&buf).unwrap().0;
    let mut ranks = Vec::with_capacity(b.size());
    for _ in 0..b.size() {
        ranks.push(range_to(vector.len())(&mut guac));
    }
    fn rank(bv: &BitVector, ranks: &[usize]) {
        for rank in ranks {
            black_box(bv).rank(black_box(*rank));
        }
    }
    b.run(|| {
        rank(&vector, &ranks);
    });
}

benchmark! {
    name = rrr_bit_vector_rank;
    RrrBitVectorParameters {
        vector_length in VECTOR_LENGTH,
        probability in BIT_PROBABILITY,
    }
    bench_bit_vector_rank
}

////////////////////////////////////////////// select //////////////////////////////////////////////

fn bench_bit_vector_select(params: &RrrBitVectorParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let mut buf = vec![];
    let mut builder = Builder::new(&mut buf);
    let vector = generate_bit_vector(params, &mut guac);
    BitVector::construct(&vector, &mut builder).unwrap();
    drop(builder);
    let vector = BitVector::parse(&buf).unwrap().0;
    let mut selects = Vec::with_capacity(b.size());
    for _ in 0..b.size() {
        selects.push(vector.rank(range_to(vector.len())(&mut guac)).unwrap());
    }
    fn select(bv: &BitVector, selects: &[usize]) {
        for select in selects {
            black_box(bv).select(black_box(*select));
        }
    }
    b.run(|| {
        select(&vector, &selects);
    });
}

benchmark! {
    name = rrr_bit_vector_select;
    RrrBitVectorParameters {
        vector_length in VECTOR_LENGTH,
        probability in BIT_PROBABILITY,
    }
    bench_bit_vector_select
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

statslicer_main! {
    rrr_bit_vector_new,
    rrr_bit_vector_access,
    rrr_bit_vector_rank,
    rrr_bit_vector_select,
}
