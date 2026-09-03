//! Criterion benchmark comparing three implementations of the M0 query slice
//! `SUM(price) WHERE date <= bound` over 1M rows of fixed-seed synthetic
//! data:
//!
//! 1. `naive_row_loop` — row-by-row loop over the same arrow arrays,
//!    simulating a row engine (per-row predicate plus accumulation).
//! 2. `flux_exec_kernel` — this crate's vectorized kernel.
//! 3. `arrow_compute` — arrow's built-in kernels (`lt_eq` + `filter` +
//!    `sum`), as the reference vectorized implementation.
//!
//! Numbers from a Windows dev box are smoke-level only: relative gaps are
//! meaningful, absolute values are not — official numbers come from the
//! Linux CI benchmark job once it exists.

use std::hint::black_box;

use arrow::compute::kernels::cmp::lt_eq;
use arrow::compute::{filter, sum};
use arrow_array::{Array, Float64Array, Int32Array};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use flux_exec::select_sum;

const ROWS: usize = 1_000_000;
/// Day-number domain is [0, 36_500) (100 years); the bound sits in the
/// middle for roughly 50% selectivity.
const BOUND: i32 = 18_250;
const PRICE_MAX: f64 = 1_000.0;

/// xorshift64* PRNG — deterministic and dependency-free, keeping `rand` out
/// of the dev-dependency set.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // The seed is forced odd: xorshift must never start from zero.
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_i32(&mut self) -> i32 {
        (self.next_u64() >> 33) as i32
    }

    fn next_f64(&mut self) -> f64 {
        // Uniform in [0, 1) with 53 mantissa bits.
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

fn synthetic_columns(rows: usize) -> (Int32Array, Float64Array) {
    let mut rng = Rng::new(0x00F1_5EED_0000_0042);
    let dates: Vec<i32> = (0..rows)
        .map(|_| rng.next_i32().rem_euclid(36_500))
        .collect();
    let prices: Vec<f64> = (0..rows).map(|_| rng.next_f64() * PRICE_MAX).collect();
    (dates.into(), prices.into())
}

/// Row-at-a-time baseline over the same columnar data.
fn naive_row_loop(dates: &Int32Array, prices: &Float64Array, bound: i32) -> f64 {
    let mut sum = 0.0;
    for i in 0..dates.len() {
        if dates.is_valid(i) && dates.value(i) <= bound && prices.is_valid(i) {
            sum += prices.value(i);
        }
    }
    sum
}

/// arrow's own kernels, as the reference vectorized implementation.
fn arrow_compute(dates: &Int32Array, prices: &Float64Array, bound: i32) -> f64 {
    let mask = lt_eq(dates, &Int32Array::new_scalar(bound)).unwrap();
    let filtered = filter(prices, &mask).unwrap();
    let filtered = filtered
        .as_any()
        .downcast_ref::<Float64Array>()
        .expect("filter preserves the value type");
    sum(filtered).unwrap_or(0.0)
}

fn bench_select_sum(c: &mut Criterion) {
    let (dates, prices) = synthetic_columns(ROWS);
    let mut group = c.benchmark_group("select_sum_1m_rows");
    group.throughput(Throughput::Elements(ROWS as u64));

    group.bench_function("naive_row_loop", |b| {
        b.iter(|| black_box(naive_row_loop(&dates, &prices, BOUND)))
    });
    group.bench_function("flux_exec_kernel", |b| {
        b.iter(|| black_box(select_sum(&dates, BOUND, &prices).unwrap()))
    });
    group.bench_function("arrow_compute", |b| {
        b.iter(|| black_box(arrow_compute(&dates, &prices, BOUND)))
    });

    group.finish();
}

criterion_group!(benches, bench_select_sum);
criterion_main!(benches);
