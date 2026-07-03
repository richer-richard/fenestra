//! `mul_div` is the exact-rational primitive a frame/tick timebase is built
//! on: it must agree with a `u128` reference for every rounding mode, and a
//! monotone tick sequence converted through it must itself be monotone with
//! zero accumulated drift, no matter how far out the sequence runs.

use fenestra_anim::{Rounding, mul_div};
use proptest::prelude::*;

/// A `u128` reference distinct from `mul_div`'s own internal structure:
/// computes the quotient and remainder separately, then resolves rounding
/// from those, rather than `mul_div`'s combined-formula approach for
/// `Round`. Returns `None` when the exact result would not fit `u64` (the
/// case `mul_div` panics on).
fn reference(a: u64, b: u64, c: u64, rounding: Rounding) -> Option<u64> {
    let product = u128::from(a) * u128::from(b);
    let c128 = u128::from(c);
    let quotient = product / c128;
    let remainder = product % c128;
    let result = match rounding {
        Rounding::Floor => quotient,
        Rounding::Ceil => {
            if remainder == 0 {
                quotient
            } else {
                quotient + 1
            }
        }
        Rounding::Round => {
            if remainder * 2 >= c128 {
                quotient + 1
            } else {
                quotient
            }
        }
    };
    u64::try_from(result).ok()
}

proptest! {
    /// Realistic timebase magnitudes (well clear of u64 overflow): exact
    /// agreement with the reference for every rounding mode.
    #[test]
    fn matches_u128_reference_at_realistic_magnitudes(
        a in 0u64..10_000_000,
        b in 1u64..1_000_000,
        c in 1u64..1_000_000,
        mode in prop_oneof![Just(Rounding::Floor), Just(Rounding::Ceil), Just(Rounding::Round)],
    ) {
        let expected = reference(a, b, c, mode).expect("realistic magnitudes never overflow u64");
        prop_assert_eq!(mul_div(a, b, c, mode), expected);
    }

    /// Full `u64` range: wherever the exact result fits `u64`, `mul_div`
    /// must still match the reference exactly (this is where overflow near
    /// the boundary, off-by-one rounding, and the `u128` widening itself
    /// get exercised).
    #[test]
    fn matches_u128_reference_at_full_u64_range(
        a in any::<u64>(),
        b in any::<u64>(),
        c in 1u64..,
        mode in prop_oneof![Just(Rounding::Floor), Just(Rounding::Ceil), Just(Rounding::Round)],
    ) {
        if let Some(expected) = reference(a, b, c, mode) {
            prop_assert_eq!(mul_div(a, b, c, mode), expected);
        }
    }
}

/// Converting a monotone tick sequence to another rate through `mul_div`
/// (recomputed fresh per tick, never accumulated) stays monotone and
/// matches the exact rational value at every sampled point — no drift, no
/// matter how far out the sequence runs. Walks a dense head (the small
/// ticks where off-by-one bugs usually hide) plus a strided tail out to
/// 10^9 ticks (a prime stride, so the samples don't alias any periodicity
/// in the 48000/60 ratio).
#[test]
fn monotone_tick_sequence_has_zero_drift_over_a_billion_ticks() {
    // 48 kHz audio ticks converted to 60 fps video frames: a ratio with no
    // common factor reduction that would mask a rounding bug.
    const NUM: u64 = 60;
    const DEN: u64 = 48_000;
    const STRIDE: u64 = 99_991; // prime, well clear of NUM/DEN's factors
    const CEILING: u64 = 1_000_000_000;

    let frame_at = |tick: u64| mul_div(tick, NUM, DEN, Rounding::Floor);

    let mut prev_tick = 0u64;
    let mut prev_frame = frame_at(0);
    assert_eq!(prev_frame, 0);

    let dense_head = 0..1_000_000u64;
    let strided_tail = (1_000_000..=CEILING).step_by(STRIDE as usize);

    for tick in dense_head.chain(strided_tail) {
        let frame = frame_at(tick);
        // Zero drift: recomputing from scratch always matches the exact
        // rational value, not an accumulated approximation of it.
        let exact = u128::from(tick) * u128::from(NUM) / u128::from(DEN);
        assert_eq!(
            u128::from(frame),
            exact,
            "tick {tick}: frame {frame} does not match the exact rational value {exact}"
        );
        // Monotone: a later tick never maps to an earlier frame.
        if tick > prev_tick {
            assert!(
                frame >= prev_frame,
                "tick {tick} (frame {frame}) went backward from tick {prev_tick} (frame {prev_frame})"
            );
        }
        prev_tick = tick;
        prev_frame = frame;
    }

    // The far end of the walk actually reached its expected magnitude —
    // guards against the stride silently degenerating to a short loop.
    assert!(prev_tick > 999_000_000, "walk stopped short: {prev_tick}");
}
