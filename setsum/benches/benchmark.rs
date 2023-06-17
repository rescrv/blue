use std::hint::black_box;

use rand::Rng;

use criterion::{criterion_group, criterion_main, Criterion};

use guacamole::Guacamole;

use setsum::{add_state, SETSUM_COLUMNS, SETSUM_PRIMES};

pub fn add_state_div(lhs: [u32; SETSUM_COLUMNS], rhs: [u32; SETSUM_COLUMNS]) -> [u32; SETSUM_COLUMNS] {
    let mut ret = <[u32; SETSUM_COLUMNS]>::default();
    for i in 0..SETSUM_COLUMNS {
        let lc = lhs[i] as u64;
        let rc = rhs[i] as u64;
        let sum = (lc + rc) % SETSUM_PRIMES[i] as u64;
        ret[i] = sum as u32;
    }
    ret
}

pub fn add_state_sub(lhs: [u32; SETSUM_COLUMNS], rhs: [u32; SETSUM_COLUMNS]) -> [u32; SETSUM_COLUMNS] {
    let mut ret = <[u32; SETSUM_COLUMNS]>::default();
    for i in 0..SETSUM_COLUMNS {
        let lc = lhs[i] as u64;
        let rc = rhs[i] as u64;
        let mut sum = lc + rc;
        let p = SETSUM_PRIMES[i] as u64;
        if sum >= p {
            sum -= p;
        }
        ret[i] = sum as u32;
    }
    ret
}

fn fold_state_div(items: &[[u32; SETSUM_COLUMNS]]) {
    let mut acc = [0u32; SETSUM_COLUMNS];
    for item in items {
        acc = add_state_div(acc, *item);
    }
    black_box(acc);
}

fn fold_state_sub(items: &[[u32; SETSUM_COLUMNS]]) {
    let mut acc = [0u32; SETSUM_COLUMNS];
    for item in items {
        acc = add_state_sub(acc, *item);
    }
    black_box(acc);
}

fn fold_state_lib(items: &[[u32; SETSUM_COLUMNS]]) {
    let mut acc = [0u32; SETSUM_COLUMNS];
    for item in items {
        acc = add_state(acc, *item);
    }
    black_box(acc);
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut guac = Guacamole::new(0);
    let mut samples = Vec::new();
    for _ in 0..100_000 {
        let mut sample: [u32; SETSUM_COLUMNS] = [0u32; SETSUM_COLUMNS];
        for s in sample.iter_mut() {
            *s = guac.gen();
        }
        samples.push(sample);
    }
    c.bench_function("fold_state_div 1", |b| b.iter(|| fold_state_div(black_box(&samples[..1]))));
    c.bench_function("fold_state_div 10", |b| b.iter(|| fold_state_div(black_box(&samples[..10]))));
    c.bench_function("fold_state_div 100", |b| b.iter(|| fold_state_div(black_box(&samples[..100]))));
    c.bench_function("fold_state_div 1_000", |b| b.iter(|| fold_state_div(black_box(&samples[..1_000]))));
    c.bench_function("fold_state_div 10_000", |b| b.iter(|| fold_state_div(black_box(&samples[..10_000]))));
    c.bench_function("fold_state_div 100_000", |b| b.iter(|| fold_state_div(black_box(&samples[..100_000]))));
    c.bench_function("fold_state_sub 1", |b| b.iter(|| fold_state_sub(black_box(&samples[..1]))));
    c.bench_function("fold_state_sub 10", |b| b.iter(|| fold_state_sub(black_box(&samples[..10]))));
    c.bench_function("fold_state_sub 100", |b| b.iter(|| fold_state_sub(black_box(&samples[..100]))));
    c.bench_function("fold_state_sub 1_000", |b| b.iter(|| fold_state_sub(black_box(&samples[..1_000]))));
    c.bench_function("fold_state_sub 10_000", |b| b.iter(|| fold_state_sub(black_box(&samples[..10_000]))));
    c.bench_function("fold_state_sub 100_000", |b| b.iter(|| fold_state_sub(black_box(&samples[..100_000]))));
    c.bench_function("fold_state_lib 1", |b| b.iter(|| fold_state_lib(black_box(&samples[..1]))));
    c.bench_function("fold_state_lib 10", |b| b.iter(|| fold_state_lib(black_box(&samples[..10]))));
    c.bench_function("fold_state_lib 100", |b| b.iter(|| fold_state_lib(black_box(&samples[..100]))));
    c.bench_function("fold_state_lib 1_000", |b| b.iter(|| fold_state_lib(black_box(&samples[..1_000]))));
    c.bench_function("fold_state_lib 10_000", |b| b.iter(|| fold_state_lib(black_box(&samples[..10_000]))));
    c.bench_function("fold_state_lib 100_000", |b| b.iter(|| fold_state_lib(black_box(&samples[..100_000]))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
