use crate::invalid;
use serde::{Deserialize, Serialize};
use std::{
    io,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const MODEL_PREPARE_MS: u64 = 90_000;
pub const MODEL_USABLE_MS: u64 = 1_200_000;
pub const MODEL_END_MS: u64 = 30_000;
pub const MODEL_ORACLE_MS: u64 = 60_000;
pub const MODEL_CLEANUP_MS: u64 = 600_000;
pub const MODEL_WORKER_MS: u64 = MODEL_PREPARE_MS + MODEL_USABLE_MS + MODEL_END_MS;
pub const MODEL_ADMISSION_MS: u64 = MODEL_WORKER_MS + MODEL_ORACLE_MS + MODEL_CLEANUP_MS;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetSpec {
    pub issued_unix_ms: u64,
    pub expires_unix_ms: u64,
    pub maximum_elapsed_ms: u64,
    pub clock_drift_tolerance_ms: u64,
}
impl BudgetSpec {
    pub fn validate(&self, now: u64) -> io::Result<()> {
        if self.issued_unix_ms == 0
            || self.expires_unix_ms <= self.issued_unix_ms
            || self.expires_unix_ms - self.issued_unix_ms > 3_600_000
            || !(1..=3_600_000).contains(&self.maximum_elapsed_ms)
            || self.clock_drift_tolerance_ms > 2000
            || now >= self.expires_unix_ms
            || now.saturating_add(self.clock_drift_tolerance_ms) < self.issued_unix_ms
        {
            return Err(invalid("invalid_expired_or_future_budget"));
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Budget {
    spec: BudgetSpec,
    start: Instant,
    wall_start: u64,
    limit: Duration,
}
impl Budget {
    pub fn start(spec: BudgetSpec) -> io::Result<Self> {
        let now = now_ms();
        spec.validate(now)?;
        let limit = Duration::from_millis(spec.maximum_elapsed_ms.min(spec.expires_unix_ms - now));
        Ok(Self {
            spec,
            start: Instant::now(),
            wall_start: now,
            limit,
        })
    }
    pub fn check(&self) -> io::Result<()> {
        self.check_at(now_ms(), self.start.elapsed())
    }
    pub fn check_at(&self, wall: u64, elapsed: Duration) -> io::Result<()> {
        let expected = self.wall_start as i128 + elapsed.as_millis() as i128;
        if (wall as i128 - expected).unsigned_abs() > self.spec.clock_drift_tolerance_ms as u128 {
            return Err(invalid("clock_drift_or_rollback_hold"));
        }
        if elapsed >= self.limit || wall >= self.spec.expires_unix_ms {
            return Err(invalid("monotonic_or_wall_deadline_hold"));
        }
        Ok(())
    }
    pub fn remaining(&self) -> Duration {
        self.limit.saturating_sub(self.start.elapsed())
    }
    pub fn deadline(&self) -> Instant {
        self.start + self.limit
    }
    pub fn stage_spec(&self, max_ms: u64) -> io::Result<BudgetSpec> {
        self.check()?;
        let now = now_ms();
        Ok(BudgetSpec {
            issued_unix_ms: now,
            expires_unix_ms: self.spec.expires_unix_ms.min(now.saturating_add(max_ms)),
            maximum_elapsed_ms: max_ms.min(self.remaining().as_millis() as u64),
            clock_drift_tolerance_ms: self.spec.clock_drift_tolerance_ms,
        })
    }
    /// Unlike stage_spec, this never clips the requested interval. Both clocks
    /// must have the complete interval and its non-dispatch tail available.
    pub fn exact_spec(&self, duration_ms: u64, tail_ms: u64) -> io::Result<BudgetSpec> {
        self.check()?;
        let now = now_ms();
        let needed = duration_ms
            .checked_add(tail_ms)
            .ok_or_else(|| invalid("budget_overflow"))?;
        if self.remaining() < Duration::from_millis(needed)
            || self.spec.expires_unix_ms.saturating_sub(now) < needed
        {
            return Err(invalid("full_model_and_fixed_reserves_unavailable"));
        }
        Ok(BudgetSpec {
            issued_unix_ms: now,
            expires_unix_ms: now + duration_ms,
            maximum_elapsed_ms: duration_ms,
            clock_drift_tolerance_ms: self.spec.clock_drift_tolerance_ms,
        })
    }
}

/// One nonrenewable admission. Preparation, usable model time, and END are
/// separate phases; all remain subordinate to the original task clocks.
pub struct ModelAdmission {
    parent: Budget,
    preparation: Budget,
    worker: BudgetSpec,
    usable: Option<Budget>,
    ending: Option<Budget>,
}
impl ModelAdmission {
    pub fn reserve(parent: &Budget) -> io::Result<Self> {
        let worker = parent.exact_spec(MODEL_WORKER_MS, MODEL_ORACLE_MS + MODEL_CLEANUP_MS)?;
        let preparation = Budget::start(BudgetSpec {
            expires_unix_ms: worker.issued_unix_ms + MODEL_PREPARE_MS,
            maximum_elapsed_ms: MODEL_PREPARE_MS,
            ..worker.clone()
        })?;
        Ok(Self {
            parent: parent.clone(),
            preparation,
            worker,
            usable: None,
            ending: None,
        })
    }
    pub fn worker_spec(&self) -> io::Result<BudgetSpec> {
        self.check()?;
        if self.usable.is_some() {
            return Err(invalid("model_worker_already_ready"));
        }
        Ok(self.worker.clone())
    }
    pub fn ready(&mut self) -> io::Result<BudgetSpec> {
        self.check()?;
        if self.usable.is_some() || self.ending.is_some() {
            return Err(invalid("model_ready_cannot_renew"));
        }
        let spec = self.parent.exact_spec(
            MODEL_USABLE_MS,
            MODEL_END_MS + MODEL_ORACLE_MS + MODEL_CLEANUP_MS,
        )?;
        if spec.expires_unix_ms > self.worker.expires_unix_ms.saturating_sub(MODEL_END_MS) {
            return Err(invalid("model_preparation_reserve_exhausted"));
        }
        self.usable = Some(Budget::start(spec.clone())?);
        Ok(spec)
    }
    pub fn end(&mut self) -> io::Result<()> {
        self.check()?;
        if self.usable.is_none() || self.ending.is_some() {
            return Err(invalid("model_end_phase_order"));
        }
        self.ending = Some(Budget::start(
            self.parent
                .exact_spec(MODEL_END_MS, MODEL_ORACLE_MS + MODEL_CLEANUP_MS)?,
        )?);
        Ok(())
    }
    pub fn check(&self) -> io::Result<()> {
        self.parent.check()?;
        if let Some(end) = &self.ending {
            end.check()
        } else if let Some(usable) = &self.usable {
            usable.check()
        } else {
            self.preparation.check()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monotonic_expiry_and_clock_changes_do_not_extend_grants() {
        let now = now_ms();
        let b = Budget::start(BudgetSpec {
            issued_unix_ms: now,
            expires_unix_ms: now + 10000,
            maximum_elapsed_ms: 9000,
            clock_drift_tolerance_ms: 100,
        })
        .unwrap();
        assert!(
            b.check_at(b.wall_start + 1000, Duration::from_secs(1))
                .is_ok()
        );
        assert!(b.check_at(b.wall_start, Duration::from_secs(1)).is_err());
        assert!(
            b.check_at(b.wall_start + 9000, Duration::from_secs(9))
                .is_err()
        );
        assert!(
            b.check_at(b.wall_start + 900, Duration::from_secs(10))
                .is_err()
        );
    }
}
