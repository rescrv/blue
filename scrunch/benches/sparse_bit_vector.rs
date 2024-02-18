use std::collections::HashSet;

use guacamole::combinators::*;
use guacamole::Guacamole;
use statslicer::{benchmark, black_box, statslicer_main, Bencher, Parameter, Parameters};

use scrunch::bit_vector::sparse::BitVector;
use scrunch::bit_vector::BitVector as BitVectorTrait;
use scrunch::builder::Builder;

const BITS_SET: &[usize] = &[4096];

const BRANCH: &[usize] = &[4, 8, 16, 32, 64, 128];

#[derive(Debug, Default, Eq, PartialEq)]
struct SparseBitVectorParameters {
    bits_set: usize,
    branch: usize,
}

impl Parameters for SparseBitVectorParameters {
    fn params(&self) -> Vec<(&'static str, Parameter)> {
        vec![
            ("bits_set", Parameter::Integer(self.bits_set as u64)),
            ("branch", Parameter::Integer(self.branch as u64)),
        ]
    }
}

/////////////////////////////////////////////// util ///////////////////////////////////////////////

fn generate_bit_vector(params: &SparseBitVectorParameters, guac: &mut Guacamole) -> Vec<usize> {
    let mut bits = HashSet::new();
    while bits.len() < params.bits_set {
        bits.insert(any::<u32>(guac) as usize);
    }
    let mut bits: Vec<usize> = bits.into_iter().collect();
    bits.sort();
    bits
}

//////////////////////////////////////////////// new ///////////////////////////////////////////////

fn bench_bit_vector_new(params: &SparseBitVectorParameters, b: &mut Bencher) {
    let mut vectors = vec![];
    let mut guac = Guacamole::new(b.seed());
    for _ in 0..b.size() {
        vectors.push(generate_bit_vector(params, &mut guac));
    }
    fn construct(params: &SparseBitVectorParameters, bits: &[usize]) -> Vec<u8> {
        let mut buf = vec![];
        let mut builder = Builder::new(&mut buf);
        BitVector::from_indices(params.branch, bits[bits.len() - 1] + 1, bits, &mut builder);
        drop(builder);
        black_box(buf)
    }
    b.run(|| {
        for vector in vectors.into_iter() {
            construct(params, black_box(&vector));
        }
    });
}

benchmark! {
    name = sparse_bit_vector_new;
    SparseBitVectorParameters {
        bits_set in BITS_SET,
        branch in BRANCH,
    }
    bench_bit_vector_new
}

////////////////////////////////////////////// access //////////////////////////////////////////////

fn bench_bit_vector_access(params: &SparseBitVectorParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let mut buf = vec![];
    let mut builder = Builder::new(&mut buf);
    let vector = generate_bit_vector(params, &mut guac);
    BitVector::from_indices(
        params.branch,
        vector[vector.len() - 1] + 1,
        &vector,
        &mut builder,
    )
    .unwrap();
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
    name = sparse_bit_vector_access;
    SparseBitVectorParameters {
        bits_set in BITS_SET,
        branch in BRANCH,
    }
    bench_bit_vector_access
}

/////////////////////////////////////////////// rank ///////////////////////////////////////////////

fn bench_bit_vector_rank(params: &SparseBitVectorParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let mut buf = vec![];
    let mut builder = Builder::new(&mut buf);
    let vector = generate_bit_vector(params, &mut guac);
    BitVector::from_indices(
        params.branch,
        vector[vector.len() - 1] + 1,
        &vector,
        &mut builder,
    )
    .unwrap();
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
    name = sparse_bit_vector_rank;
    SparseBitVectorParameters {
        bits_set in BITS_SET,
        branch in BRANCH,
    }
    bench_bit_vector_rank
}

////////////////////////////////////////////// select //////////////////////////////////////////////

fn bench_bit_vector_select(params: &SparseBitVectorParameters, b: &mut Bencher) {
    let mut guac = Guacamole::new(b.seed());
    let mut buf = vec![];
    let mut builder = Builder::new(&mut buf);
    let vector = generate_bit_vector(params, &mut guac);
    BitVector::from_indices(
        params.branch,
        vector[vector.len() - 1] + 1,
        &vector,
        &mut builder,
    )
    .unwrap();
    drop(builder);
    let vector = BitVector::parse(&buf).unwrap().0;
    let mut selects = Vec::with_capacity(b.size());
    let max_rank = vector.rank(vector.len()).unwrap();
    for _ in 0..b.size() {
        selects.push(range_to(max_rank + 1)(&mut guac));
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
    name = sparse_bit_vector_select;
    SparseBitVectorParameters {
        bits_set in BITS_SET,
        branch in BRANCH,
    }
    bench_bit_vector_select
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

statslicer_main! {
    sparse_bit_vector_new,
    sparse_bit_vector_access,
    sparse_bit_vector_rank,
    sparse_bit_vector_select,
}
