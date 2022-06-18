use std::fmt::Debug;
use std::time::Instant;

use biometrics::{Counter,Gauge,Moments,Sensor};
use prototk::Message;

///////////////////////////////////////////// Condition ////////////////////////////////////////////

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
enum Condition {
    Firing,
    Stable,
}

//////////////////////////////////////////// SuccessRate ///////////////////////////////////////////

#[derive(Clone,Debug,Default)]
pub struct SuccessState {
    success: u64,
    failure: u64,
}

pub struct SuccessRate<'a> {
    label: &'static str,
    success: &'a Counter,
    failure: &'a Counter,
    threshold: f64,
}

impl<'a> SuccessRate<'a> {
    pub const fn new(label: &'static str, success: &'a Counter, failure: &'a Counter, threshold: f64) -> Self {
        SuccessRate {
            label,
            success,
            failure,
            threshold,
        }
    }

    fn evaluate(&self, previous_state: &mut SuccessState) -> Condition {
        let current_state = SuccessState {
            success: self.success.read(),
            failure: self.failure.read(),
        };
        if current_state.success < previous_state.success {
            return Condition::Firing;
        }
        if current_state.success - previous_state.success >= (1u64<<56) {
            return Condition::Firing;
        }
        if current_state.failure < previous_state.failure {
            return Condition::Firing;
        }
        if current_state.failure - previous_state.failure >= (1u64<<56)  {
            return Condition::Firing;
        }
        let success = (current_state.success - previous_state.success) as f64;
        let failure = (current_state.failure - previous_state.failure) as f64;
        if success + failure <= 0.0 {
            return Condition::Firing;
        }
        *previous_state = current_state;
        let success_rate = success / (success + failure);
        if success_rate >= self.threshold {
            Condition::Stable
        } else {
            Condition::Firing
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_rate() {
        static COUNTER1: Counter = Counter::new();
        static COUNTER2: Counter = Counter::new();
        static SUCCESS: SuccessRate = SuccessRate::new("success.rate.test", &COUNTER1, &COUNTER2, 0.999);

        let mut state = SuccessState::default();

        assert_eq!(0, COUNTER1.read());
        assert_eq!(0, COUNTER2.read());
        assert_eq!(Condition::Firing, SUCCESS.evaluate(&mut state));

        for _ in 0..999 {
            COUNTER1.click();
        }
        COUNTER2.click();

        assert_eq!(999, COUNTER1.read());
        assert_eq!(1, COUNTER2.read());
        assert_eq!(Condition::Stable, SUCCESS.evaluate(&mut state));
    }
}
