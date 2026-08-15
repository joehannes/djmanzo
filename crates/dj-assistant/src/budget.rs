//! A hard limit on what a session can spend.
//!
//! Voice-driven DJing can issue a great many requests, and the failure mode is
//! not subtle: a wake word that mistriggers on a vocal sample, four hours of
//! set, and a bill in the morning. So the cap is checked **before** each
//! request rather than reported after, and refusing is the default when the
//! cost of a call is unknown.

use std::sync::atomic::{AtomicU64, Ordering};

/// Money as whole micro-dollars, so it can live in an atomic and be read from
/// anywhere without a lock. Floating-point money in a running total also
/// accumulates error, which is the wrong property for something that decides
/// whether to refuse.
type MicroUsd = u64;

const MICROS_PER_USD: f64 = 1_000_000.0;

#[derive(Debug)]
pub struct Budget {
    cap_micros: AtomicU64,
    spent_micros: AtomicU64,
    /// Calls whose cost the provider never reported.
    unpriced_calls: AtomicU64,
}

impl Default for Budget {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAP_USD)
    }
}

impl Budget {
    /// Enough for an evening of real use, small enough that a stuck wake word
    /// is an annoyance rather than an incident.
    pub const DEFAULT_CAP_USD: f64 = 2.00;

    #[must_use]
    pub fn new(cap_usd: f64) -> Self {
        Self {
            cap_micros: AtomicU64::new(to_micros(cap_usd)),
            spent_micros: AtomicU64::new(0),
            unpriced_calls: AtomicU64::new(0),
        }
    }

    /// A budget that never refuses. For local models, which cost nothing.
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            cap_micros: AtomicU64::new(u64::MAX),
            spent_micros: AtomicU64::new(0),
            unpriced_calls: AtomicU64::new(0),
        }
    }

    pub fn set_cap(&self, cap_usd: f64) {
        self.cap_micros.store(to_micros(cap_usd), Ordering::Relaxed);
    }

    #[must_use]
    pub fn cap_usd(&self) -> f64 {
        let cap = self.cap_micros.load(Ordering::Relaxed);
        if cap == u64::MAX {
            f64::INFINITY
        } else {
            cap as f64 / MICROS_PER_USD
        }
    }

    #[must_use]
    pub fn spent_usd(&self) -> f64 {
        self.spent_micros.load(Ordering::Relaxed) as f64 / MICROS_PER_USD
    }

    #[must_use]
    pub fn remaining_usd(&self) -> f64 {
        (self.cap_usd() - self.spent_usd()).max(0.0)
    }

    /// How many calls could not be priced.
    ///
    /// Surfaced because a session whose spend reads `$0.00` after fifty calls
    /// is reporting ignorance, not thrift, and the interface should say which.
    #[must_use]
    pub fn unpriced_calls(&self) -> u64 {
        self.unpriced_calls.load(Ordering::Relaxed)
    }

    /// Whether another request is allowed.
    #[must_use]
    pub fn allows_another(&self) -> bool {
        self.spent_micros.load(Ordering::Relaxed) < self.cap_micros.load(Ordering::Relaxed)
    }

    /// Record what a call cost. `None` means the provider did not say.
    pub fn record(&self, cost_usd: Option<f64>) {
        match cost_usd {
            Some(cost) if cost.is_finite() && cost > 0.0 => {
                self.spent_micros
                    .fetch_add(to_micros(cost), Ordering::Relaxed);
            }
            Some(_) => {}
            None => {
                self.unpriced_calls.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Forget the running total. Called when a set starts.
    pub fn reset(&self) {
        self.spent_micros.store(0, Ordering::Relaxed);
        self.unpriced_calls.store(0, Ordering::Relaxed);
    }
}

fn to_micros(usd: f64) -> MicroUsd {
    if !usd.is_finite() || usd <= 0.0 {
        return 0;
    }
    (usd * MICROS_PER_USD).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_budget_allows_spending() {
        let budget = Budget::new(1.0);
        assert!(budget.allows_another());
        assert_eq!(budget.spent_usd(), 0.0);
        assert!((budget.remaining_usd() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn spending_accumulates_and_eventually_refuses() {
        let budget = Budget::new(0.10);
        for _ in 0..9 {
            budget.record(Some(0.01));
        }
        assert!(
            budget.allows_another(),
            "9c of a 10c cap should still allow"
        );

        budget.record(Some(0.01));
        assert!(!budget.allows_another(), "the cap should now be reached");
        assert!(budget.remaining_usd() < 1e-9);
    }

    /// The cap must hold when several requests are in flight at once, which is
    /// exactly what voice control produces.
    #[test]
    fn concurrent_recording_does_not_lose_spend() {
        use std::sync::Arc;
        let budget = Arc::new(Budget::new(100.0));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let budget = Arc::clone(&budget);
            handles.push(std::thread::spawn(move || {
                for _ in 0..1000 {
                    budget.record(Some(0.001));
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        // 8 threads x 1000 x $0.001 = $8.00, exactly.
        assert!(
            (budget.spent_usd() - 8.0).abs() < 1e-6,
            "{}",
            budget.spent_usd()
        );
    }

    /// Unknown cost is counted as unknown, not as free. A session reading
    /// `$0.00` after fifty calls should say it does not know.
    #[test]
    fn unpriced_calls_are_counted_rather_than_treated_as_free() {
        let budget = Budget::new(1.0);
        budget.record(None);
        budget.record(None);
        assert_eq!(budget.spent_usd(), 0.0);
        assert_eq!(budget.unpriced_calls(), 2);
    }

    #[test]
    fn an_unlimited_budget_never_refuses() {
        let budget = Budget::unlimited();
        budget.record(Some(1_000_000.0));
        assert!(budget.allows_another());
        assert!(budget.cap_usd().is_infinite());
    }

    #[test]
    fn resetting_starts_the_session_over() {
        let budget = Budget::new(0.01);
        budget.record(Some(0.02));
        budget.record(None);
        assert!(!budget.allows_another());

        budget.reset();
        assert!(budget.allows_another());
        assert_eq!(budget.unpriced_calls(), 0);
    }

    /// A cap of zero or a nonsense cap must refuse rather than allow
    /// everything, because the safe direction is obvious.
    #[test]
    fn a_nonsense_cap_refuses_rather_than_permits() {
        for cap in [0.0, -5.0, f64::NAN] {
            let budget = Budget::new(cap);
            assert!(!budget.allows_another(), "cap {cap} should refuse");
        }
    }

    #[test]
    fn the_cap_can_be_raised_mid_session() {
        let budget = Budget::new(0.01);
        budget.record(Some(0.02));
        assert!(!budget.allows_another());

        budget.set_cap(5.0);
        assert!(budget.allows_another(), "raising the cap should resume");
        // And the spend so far is remembered, not forgotten.
        assert!((budget.spent_usd() - 0.02).abs() < 1e-9);
    }
}
