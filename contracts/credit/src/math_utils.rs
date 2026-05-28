// SPDX-License-Identifier: MIT

//! Pure integer arithmetic helpers used across the credit contract.

#![warn(missing_docs)]

/// Multiply `value` by `numerator` then divide by `denominator`, using an
/// intermediate `i128` accumulator to avoid overflow on typical inputs.
///
/// # Rounding
/// Truncates toward zero (floor for positive results). No rounding-up variant
/// is provided; callers that need ceiling arithmetic should add
/// `denominator - 1` to `value * numerator` before calling.
///
/// # Parameters
/// - `value`:       The base amount to scale.
/// - `numerator`:   Scaling numerator (e.g. an interest rate).
/// - `denominator`: Scaling denominator (e.g. 10_000 for basis-point math).
///
/// # Returns
/// `(value * numerator) / denominator`, truncated toward zero.
///
/// # Panics
/// - If `denominator` is zero (division by zero).
/// - If the intermediate product `value * numerator` overflows `i128`
///   (unlikely in practice; `i128` supports values up to ~1.7 × 10³⁸).
///
/// # Example
/// ```
/// // 1_000 * 300 / 10_000 = 30  (3% of 1_000)
/// assert_eq!(mul_div(1_000, 300, 10_000), 30);
/// ```
// (Old, simpler i128 helpers removed in favor of the fixed-point, rounding-aware
// implementations below.)

/// Legacy i128 helpers retained for compatibility with existing code/tests.
/// These mirror the previous simple implementations and preserve truncating
/// behavior (toward zero).
pub fn mul_div(value: i128, numerator: i128, denominator: i128) -> i128 {
    assert!(denominator != 0, "mul_div: denominator must not be zero");
    value
        .checked_mul(numerator)
        .expect("mul_div: intermediate product overflowed i128")
        / denominator
}

// Ensure module-level braces are balanced.

}

/// Legacy apply_bps that operates on `i128` values and truncates toward zero.
pub fn apply_bps(amount: i128, rate_bps: u32) -> i128 {
    mul_div(amount, rate_bps as i128, 10_000)
}

/// Legacy prorate_interest using i128 arithmetic (365-day year).
pub fn prorate_interest(principal: i128, rate_bps: u32, elapsed_secs: u64) -> i128 {
    const SECONDS_PER_YEAR: i128 = 31_536_000;
    const BPS_DENOMINATOR: i128 = 10_000;

    if elapsed_secs == 0 || principal == 0 {
        return 0;
    }

    let numerator = principal
        .checked_mul(rate_bps as i128)
        .expect("prorate_interest: principal * rate_bps overflowed i128")
        .checked_mul(elapsed_secs as i128)
        .expect("prorate_interest: product with elapsed_secs overflowed i128");

    let denominator = BPS_DENOMINATOR
        .checked_mul(SECONDS_PER_YEAR)
        .expect("prorate_interest: denominator overflowed i128");

    numerator / denominator
}

//! # Fixed-Point Interest Math Utilities
//!
//! This module provides deterministic, integer-only arithmetic helpers for
//! computing interest accruals inside the Creditra credit contract.
//!
//! ## Scaling Factor
//!
//! All intermediate products are scaled by `SCALE = 10^18` before division so
//! that the final result retains sub-unit precision up to 18 decimal places.
//! The caller chooses whether the remainder is discarded (floor) or rounded up
//! (ceiling) via the [`Rounding`] enum.
//!
//! ## Basis Points
//!
//! Interest rates are expressed in **basis points** (bps), where
//! `1 bps = 0.01% = 1 / 10_000`.  The annual rate in bps is therefore divided
//! by `BPS_DENOMINATOR = 10_000` when computing the fractional rate.
//!
//! ## Annual Seconds
//!
//! Time is measured in ledger seconds.  One Julian year is defined as
//! `SECONDS_PER_YEAR = 31_557_600` (365.25 × 86 400), matching the convention
//! used by most on-chain interest protocols.
//!
//! ## Overflow Safety
//!
//! The prorate helper promotes all operands to `u128` before multiplying.
//! The worst-case intermediate product is:
//!
//! ```text
//! principal  ≤ i128::MAX  ≈ 1.7 × 10^38
//! rate_bps   ≤ 10_000
//! time_delta ≤ u64::MAX   ≈ 1.8 × 10^19
//! SCALE      = 10^18
//! ```
//!
//! `principal × rate_bps × time_delta` can reach ~3 × 10^61, which overflows
//! `u128` (max ~3.4 × 10^38).  To prevent this the multiplication is split
//! into two checked steps:
//!
//! 1. `a = principal × rate_bps`  — fits in u128 for any realistic principal
//!    (≤ 10^28 × 10^4 = 10^32 < 10^38).
//! 2. `b = a × time_delta`        — checked; panics on overflow.
//!
//! The denominator `BPS_DENOMINATOR × SECONDS_PER_YEAR` is pre-computed as a
//! `u128` constant so the final division is a single operation.

#![allow(dead_code)]

/// Scaling factor used for fixed-point intermediate arithmetic (10^18).
pub const SCALE: u128 = 1_000_000_000_000_000_000_u128;

/// Number of basis points in 100%.
pub const BPS_DENOMINATOR: u128 = 10_000;

/// Seconds in a 365-day year.
pub const SECONDS_PER_YEAR: u128 = 31_536_000;

const BPS_YEAR_DENOMINATOR: u128 = BPS_DENOMINATOR * SECONDS_PER_YEAR;

/// Rounding mode for integer division helpers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rounding {
    /// Truncate the remainder.
    Floor,
    /// Round up when a non-zero remainder exists.
    Ceil,
}

/// Multiply `value` by `numerator` and divide by `denominator`.
///
/// The result is rounded according to `rounding`.
pub fn mul_div(value: u128, numerator: u128, denominator: u128, rounding: Rounding) -> u128 {
    assert!(denominator != 0, "math_utils: division by zero");

    let product = value
        .checked_mul(numerator)
        .expect("math_utils: multiplication overflow");
    let quotient = product / denominator;

    match rounding {
        Rounding::Floor => quotient,
        Rounding::Ceil => {
            if product % denominator == 0 {
                quotient
            } else {
                quotient.checked_add(1).expect("math_utils: ceil overflow")
            }
        }
    }
}

/// Multiply `amount` by `SCALE`.
pub fn scale_up(amount: u128) -> u128 {
    amount
        .checked_mul(SCALE)
        .expect("math_utils: scale_up overflow")
}

/// Divide `amount` by `SCALE` using the requested rounding mode.
pub fn scale_down(amount: u128, rounding: Rounding) -> u128 {
    let quotient = amount / SCALE;

    match rounding {
        Rounding::Floor => quotient,
        Rounding::Ceil => {
            if amount % SCALE == 0 {
                quotient
            } else {
                quotient
                    .checked_add(1)
                    .expect("math_utils: scale_down ceil overflow")
            }
        }
    }
}

/// Apply a basis-point rate to an amount.
pub fn apply_bps(amount: u128, rate_bps: u32, rounding: Rounding) -> u128 {
    mul_div(amount, rate_bps as u128, BPS_DENOMINATOR, rounding)
}

/// Compute prorated interest for an elapsed time interval.
pub fn prorate_interest(
    principal: u128,
    rate_bps: u32,
    elapsed_secs: u64,
    rounding: Rounding,
) -> u128 {
    if principal == 0 || rate_bps == 0 || elapsed_secs == 0 {
        return 0;
    }

    let step1 = principal
        .checked_mul(rate_bps as u128)
        .expect("math_utils: prorate step1 overflow");
    let step2 = step1
        .checked_mul(elapsed_secs as u128)
        .expect("math_utils: prorate step2 overflow");

    let quotient = step2 / BPS_YEAR_DENOMINATOR;
    match rounding {
        Rounding::Floor => quotient,
        Rounding::Ceil => {
            if step2 % BPS_YEAR_DENOMINATOR == 0 {
                quotient
            } else {
                quotient
                    .checked_add(1)
                    .expect("math_utils: prorate ceil overflow")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_div_floor_and_ceil_behave_as_expected() {
        assert_eq!(mul_div(1_000, 3, 10, Rounding::Floor), 300);
        assert_eq!(mul_div(1_001, 3, 10, Rounding::Floor), 300);
        assert_eq!(mul_div(1_001, 3, 10, Rounding::Ceil), 301);
    }

    #[test]
    fn apply_bps_matches_basic_basis_point_math() {
        assert_eq!(apply_bps(10_000, 300, Rounding::Floor), 300);
        assert_eq!(apply_bps(1, 1, Rounding::Floor), 0);
        assert_eq!(apply_bps(1, 1, Rounding::Ceil), 1);
    }

    #[test]
    fn prorate_interest_handles_zero_inputs() {
        assert_eq!(prorate_interest(0, 300, 86_400, Rounding::Floor), 0);
        assert_eq!(prorate_interest(10_000, 0, 86_400, Rounding::Floor), 0);
        assert_eq!(prorate_interest(10_000, 300, 0, Rounding::Floor), 0);
    }

    #[test]
    fn prorate_interest_matches_one_year_example() {
        assert_eq!(
            prorate_interest(10_000, 300, SECONDS_PER_YEAR as u64, Rounding::Floor),
            300
        );
    }

    #[test]
    fn scale_helpers_round_trip_on_exact_values() {
        let scaled = scale_up(42);
        assert_eq!(scale_down(scaled, Rounding::Floor), 42);
        assert_eq!(scale_down(scaled, Rounding::Ceil), 42);
    }
}
