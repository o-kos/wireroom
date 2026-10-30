use dmd_protocol::{Rate, Speed};
use std::time::Duration;

pub struct Budget {
    remaining: f64,
}
impl Budget {
    pub fn new(from: u64, to: u64, rate: Rate) -> Self {
        Self {
            remaining: (to - from) as f64 * rate.denominator as f64 / rate.numerator as f64,
        }
    }
    pub fn credit(&mut self, source_seconds: f64) {
        self.remaining = (self.remaining - source_seconds).max(0.0);
    }
    pub fn ready(&self) -> bool {
        self.remaining <= 0.0
    }
}
pub fn source_time(elapsed: Duration, snapshot: &dmd_protocol::Snapshot) -> f64 {
    if snapshot.state == dmd_protocol::State::Paused {
        return 0.0;
    }
    match snapshot.speed {
        Speed::Max => f64::INFINITY,
        Speed::Numeric(speed) => elapsed.as_secs_f64() * speed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn consume(budget: &mut Budget, elapsed: Duration, speed: Speed, playing: bool) {
        let snapshot = dmd_protocol::Snapshot {
            state: if playing {
                dmd_protocol::State::Playing
            } else {
                dmd_protocol::State::Paused
            },
            frame: 0,
            speed,
            freq: 0.0,
            params: Default::default(),
        };
        budget.credit(source_time(elapsed, &snapshot));
    }
    #[test]
    fn remaining_time_survives_pause_and_speed_change() {
        let mut budget = Budget::new(
            4,
            8,
            Rate {
                numerator: 4,
                denominator: 1,
            },
        );
        consume(
            &mut budget,
            Duration::from_millis(500),
            Speed::Numeric(1.0),
            true,
        );
        consume(
            &mut budget,
            Duration::from_secs(99),
            Speed::Numeric(1.0),
            false,
        );
        consume(
            &mut budget,
            Duration::from_millis(249),
            Speed::Numeric(2.0),
            true,
        );
        assert!(!budget.ready());
        consume(
            &mut budget,
            Duration::from_millis(2),
            Speed::Numeric(2.0),
            true,
        );
        assert!(budget.ready());
    }
    #[test]
    fn exact_deltas_and_max() {
        let rate = Rate {
            numerator: 1,
            denominator: u64::MAX,
        };
        assert!(Budget::new(u64::MAX, u64::MAX, rate).ready());
        let mut budget = Budget::new(
            u64::MAX - 1,
            u64::MAX,
            Rate {
                numerator: 1,
                denominator: 1,
            },
        );
        consume(
            &mut budget,
            Duration::from_millis(500),
            Speed::Numeric(1.0),
            true,
        );
        assert_eq!(budget.remaining, 0.5);
        consume(&mut budget, Duration::ZERO, Speed::Max, false);
        assert!(!budget.ready());
        consume(&mut budget, Duration::ZERO, Speed::Max, true);
        assert!(budget.ready());
        consume(&mut budget, Duration::ZERO, Speed::Numeric(1.0), true);
        assert!(budget.ready());
    }
}
