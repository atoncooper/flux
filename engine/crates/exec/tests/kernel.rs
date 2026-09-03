//! Integration tests for the vectorized predicate kernel.
//!
//! The six M0 scenarios: empty batch, all NULL, all selected, all
//! eliminated, boundary values (inclusive `<=`), and length mismatch plus
//! f64 overflow to `inf`.

use arrow_array::{Array, BooleanArray, Float64Array, Int32Array};
use flux_exec::{KernelError, cmp_lt_scalar, select_sum, sum_masked};

/// Materializes a mask as plain bools (NULL mask entries read as `false`).
fn mask_bits(mask: &BooleanArray) -> Vec<bool> {
    mask.iter().map(|b| b.unwrap_or(false)).collect()
}

#[test]
fn select_sum_empty_batch_returns_zero() {
    let dates: Int32Array = Vec::<Option<i32>>::new().into();
    let prices: Float64Array = Vec::<Option<f64>>::new().into();

    let mask = cmp_lt_scalar(&dates, 10).unwrap();
    assert_eq!(mask.len(), 0);

    assert_eq!(sum_masked(&prices, &mask).unwrap(), 0.0);
    assert_eq!(select_sum(&dates, 10, &prices).unwrap(), 0.0);
}

#[test]
fn cmp_lt_scalar_all_null_rows_are_eliminated() {
    let dates: Int32Array = vec![None, None, None].into();
    let mask = cmp_lt_scalar(&dates, 10).unwrap();

    assert_eq!(mask_bits(&mask), vec![false, false, false]);
    assert_eq!(mask.null_count(), 0);
}

#[test]
fn sum_masked_all_null_values_are_skipped() {
    let prices: Float64Array = vec![None, None, None].into();
    let all_true = BooleanArray::from(vec![true, true, true]);

    assert_eq!(sum_masked(&prices, &all_true).unwrap(), 0.0);
}

#[test]
fn select_sum_all_null_inputs_returns_zero() {
    let dates: Int32Array = vec![None, None].into();
    let prices: Float64Array = vec![None, None].into();

    assert_eq!(select_sum(&dates, 10, &prices).unwrap(), 0.0);
}

#[test]
fn select_sum_all_selected_sums_every_value() {
    let dates: Int32Array = vec![Some(1), Some(2), Some(3)].into();
    let prices: Float64Array = vec![Some(1.5), Some(2.5), Some(3.0)].into();

    let mask = cmp_lt_scalar(&dates, 3).unwrap();
    assert_eq!(mask_bits(&mask), vec![true, true, true]);

    assert_eq!(select_sum(&dates, 3, &prices).unwrap(), 7.0);
}

#[test]
fn select_sum_all_eliminated_returns_zero() {
    let dates: Int32Array = vec![Some(11), Some(12)].into();
    let prices: Float64Array = vec![Some(1.0), Some(2.0)].into();

    let mask = cmp_lt_scalar(&dates, 10).unwrap();
    assert_eq!(mask_bits(&mask), vec![false, false]);

    assert_eq!(select_sum(&dates, 10, &prices).unwrap(), 0.0);
}

#[test]
fn cmp_lt_scalar_boundary_equal_is_selected() {
    // Inclusive comparison: `value == bound` selects the row, while the
    // NULL row next to it stays eliminated.
    let dates: Int32Array = vec![Some(4), Some(5), Some(6), None].into();
    let mask = cmp_lt_scalar(&dates, 5).unwrap();
    assert_eq!(mask_bits(&mask), vec![true, true, false, false]);

    let prices: Float64Array = vec![Some(1.0), Some(2.0), Some(4.0), Some(8.0)].into();
    assert_eq!(select_sum(&dates, 5, &prices).unwrap(), 3.0);
}

#[test]
fn select_sum_length_mismatch_returns_error() {
    let dates: Int32Array = vec![Some(1), Some(2), Some(3)].into();
    let prices: Float64Array = vec![Some(1.0), Some(2.0)].into();

    assert_eq!(
        select_sum(&dates, 5, &prices),
        Err(KernelError::LengthMismatch { left: 3, right: 2 })
    );

    // sum_masked validates lengths on its own as well.
    let mask = cmp_lt_scalar(&dates, 5).unwrap();
    let longer_prices: Float64Array = vec![Some(1.0), Some(2.0), Some(3.0), Some(4.0)].into();
    assert_eq!(
        sum_masked(&longer_prices, &mask),
        Err(KernelError::LengthMismatch { left: 4, right: 3 })
    );
}

#[test]
fn sum_masked_f64_overflow_saturates_to_inf() {
    // Documented behavior (decision 3): IEEE 754 semantics, not an error.
    let prices: Float64Array = vec![Some(f64::MAX), Some(f64::MAX)].into();
    let mask = BooleanArray::from(vec![true, true]);

    assert_eq!(sum_masked(&prices, &mask), Ok(f64::INFINITY));
}
