//! # flux-exec — vectorized predicate kernel (M0 minimal slice)
//!
//! flux-exec is the first testable module of Flux's hand-written vectorized
//! execution engine. Its purpose is to validate decision **D2** (fully
//! self-developed Rust kernel, see `CLAUDE.md`) with the smallest possible
//! amount of code: implement the two core vectorized actions — a column-wide
//! comparison producing a selection mask, and a masked aggregation — then
//! measure the real performance level against a naive row-by-row loop and
//! against arrow's built-in compute kernels (`benches/kernel.rs`).
//!
//! ## Module positioning
//!
//! - Inputs and outputs use arrow columnar types (`Int32Array`,
//!   `Float64Array`, `BooleanArray`), but the kernel logic itself is
//!   hand-written and never delegates to arrow's compute kernels.
//! - Only one minimal query slice is covered for now:
//!   `SUM(price) WHERE date <= bound`, exposed as the three pure functions
//!   in [`kernel`].
//! - No scans, Parquet, object storage, or string columns yet — those belong
//!   to later milestones and spikes.
//!
//! ## Implementation decisions
//!
//! 1. **Mask representation**: reuse arrow's bit-packed [`BooleanArray`]
//!    (8192 rows ≈ 1 KiB) instead of building a custom bitmap.
//! 2. **NULL semantics**: aligned with SQL three-valued logic —
//!    `NULL <= x` is not true, so NULL rows are eliminated (mask `false`);
//!    the mask itself never contains NULLs; [`sum_masked`] skips NULL
//!    values.
//! 3. **Overflow policy**: f64 sums saturate to `inf` instead of erroring
//!    (IEEE 754 semantics, pinned by a unit test). Future integer sums will
//!    use checked arithmetic and raise errors from the 3xxx execution-error
//!    segment.
//! 4. **Dispatch shape**: no `dyn` on hot paths (AR rules); only the i32/f64
//!    pair is implemented, without generic abstraction — generalize once a
//!    second type combination appears.
//!
//! Explicit SIMD intrinsics are deliberately out of scope for this first
//! slice: the goal is to measure the compiler's autovectorization level.
//! The hot loops therefore process data one 64-bit word at a time (packing
//! 64 row results per `u64`, aggregating from decoded mask words), which is
//! the granularity LLVM can vectorize from safe code. Hand-written
//! intrinsics are a separate follow-up decision (docs/06 §3.4 requires a
//! criterion-proven gain of at least 15%).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod kernel;

pub use kernel::{KernelError, cmp_lt_scalar, select_sum, sum_masked};
