//! Vectorized predicate kernel: three pure functions covering the minimal
//! query slice `SUM(price) WHERE date <= bound`.
//!
//! - [`cmp_lt_scalar`] produces the selection mask for the predicate,
//! - [`sum_masked`] aggregates the values selected by a mask,
//! - [`select_sum`] composes the two.
//!
//! Hot paths operate on packed 64-bit words (the comparison packs 64 row
//! results per `u64`; the aggregation decodes row indices from those words),
//! matching the granularity the compiler can autovectorize — the same
//! pattern arrow's own kernels use internally, here with safe code only.

use arrow_array::{Array, BooleanArray, Float64Array, Int32Array};
use arrow_buffer::BooleanBuffer;

/// Errors raised by the kernel functions.
///
/// M0 keeps this error type local to the crate because `flux-common` (the
/// future home of `FluxError` and the 3xxx execution-error segment) does not
/// exist yet; the variants will move there without semantic changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// Two input columns have different lengths.
    LengthMismatch {
        /// Length of the left column.
        left: usize,
        /// Length of the right column.
        right: usize,
    },
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelError::LengthMismatch { left, right } => {
                write!(
                    f,
                    "input columns have different lengths: left = {left}, right = {right}"
                )
            }
        }
    }
}

impl std::error::Error for KernelError {}

/// Packs `values[i] <= bound` into a bit buffer, 64 rows per `u64` word.
///
/// Bits at NULL positions are not cleared here — raw slots hold arbitrary
/// values, and callers clear those bits by ANDing the validity bitmap.
fn pack_lte_bits(values: &[i32], bound: i32) -> BooleanBuffer {
    let mut words = Vec::with_capacity(values.len().div_ceil(64));
    for chunk in values.chunks(64) {
        let mut word = 0u64;
        for (bit, &v) in chunk.iter().enumerate() {
            word |= u64::from(v <= bound) << bit;
        }
        words.push(word);
    }
    BooleanBuffer::new(words.into(), 0, values.len())
}

/// Sums `values` at the row indices selected by the packed `selection` bits.
fn sum_selected(values: &[f64], selection: &BooleanBuffer) -> f64 {
    let mut sum = 0.0;
    for idx in selection.set_indices() {
        sum += values[idx];
    }
    sum
}

/// Compares every row of `values` against `bound` and produces the selection
/// mask: row `i` is selected when `values[i] <= bound`.
///
/// NULL rows follow SQL three-valued logic (`NULL <= x` is not true) and are
/// eliminated, so the returned mask contains no NULLs. The name follows the
/// M0 plan; the comparison itself is inclusive (`<=`).
///
/// # Examples
///
/// ```
/// use arrow_array::Int32Array;
/// use flux_exec::cmp_lt_scalar;
///
/// let dates: Int32Array = vec![Some(1), Some(5), None, Some(9)].into();
/// let mask = cmp_lt_scalar(&dates, 5).unwrap();
/// let selected: Vec<bool> = mask.iter().map(|b| b.unwrap_or(false)).collect();
/// assert_eq!(selected, vec![true, true, false, false]);
/// ```
pub fn cmp_lt_scalar(values: &Int32Array, bound: i32) -> Result<BooleanArray, KernelError> {
    let bits = pack_lte_bits(values.values(), bound);
    // A BooleanArray is a BooleanBuffer plus an optional NullBuffer; NULL
    // rows are already encoded as `false`, so the null buffer stays `None`.
    let mask = match values.nulls() {
        Some(nulls) => &bits & nulls.inner(),
        None => bits,
    };
    Ok(BooleanArray::new(mask, None))
}

/// Sums `values`, counting only rows where the mask is `true` and the value
/// is not NULL.
///
/// NULL mask entries are treated as `false` (masks produced by this crate
/// never contain NULLs; this is defensive for externally built masks). An
/// overflowing sum saturates to `inf` per IEEE 754 instead of erroring
/// (decision 3); empty or fully masked-out input returns `0.0`.
///
/// # Errors
///
/// Returns [`KernelError::LengthMismatch`] when `values` and `mask` have
/// different lengths.
///
/// # Examples
///
/// ```
/// use arrow_array::{BooleanArray, Float64Array};
/// use flux_exec::sum_masked;
///
/// let prices: Float64Array = vec![Some(1.0), Some(2.0), None, Some(4.0)].into();
/// let mask = BooleanArray::from(vec![true, true, false, true]);
/// assert_eq!(sum_masked(&prices, &mask).unwrap(), 7.0);
/// ```
pub fn sum_masked(values: &Float64Array, mask: &BooleanArray) -> Result<f64, KernelError> {
    if values.len() != mask.len() {
        return Err(KernelError::LengthMismatch {
            left: values.len(),
            right: mask.len(),
        });
    }
    // Effective selection = mask value AND mask valid AND value valid.
    match (mask.nulls(), values.nulls()) {
        (None, None) => Ok(sum_selected(values.values(), mask.values())),
        (mask_nulls, value_nulls) => {
            let mut selection = match mask_nulls {
                Some(nulls) => mask.values() & nulls.inner(),
                None => mask.values().clone(),
            };
            if let Some(nulls) = value_nulls {
                selection = &selection & nulls.inner();
            }
            Ok(sum_selected(values.values(), &selection))
        }
    }
}

/// Composes [`cmp_lt_scalar`] and [`sum_masked`]:
/// `SUM(prices) WHERE dates <= bound`.
///
/// # Errors
///
/// Returns [`KernelError::LengthMismatch`] when `dates` and `prices` have
/// different lengths.
///
/// # Examples
///
/// ```
/// use arrow_array::{Float64Array, Int32Array};
/// use flux_exec::select_sum;
///
/// let dates: Int32Array = vec![Some(1), Some(2), None].into();
/// let prices: Float64Array = vec![Some(10.0), Some(20.0), Some(30.0)].into();
/// assert_eq!(select_sum(&dates, 2, &prices).unwrap(), 30.0);
/// ```
pub fn select_sum(
    dates: &Int32Array,
    bound: i32,
    prices: &Float64Array,
) -> Result<f64, KernelError> {
    if dates.len() != prices.len() {
        return Err(KernelError::LengthMismatch {
            left: dates.len(),
            right: prices.len(),
        });
    }
    let mask = cmp_lt_scalar(dates, bound)?;
    sum_masked(prices, &mask)
}
