use std::collections::hash_map::HashMap;
use std::fmt::Debug;

use biometrics::Counter;

///////////////////////////////////////////// Condition ////////////////////////////////////////////

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
enum Condition {
    Firing,
    Stable,
}

//////////////////////////////////////////// SuccessRate ///////////////////////////////////////////

#[derive(Clone,Debug,Default)]
pub struct SuccessRateState {
    success: u64,
    failure: u64,
}

pub struct SuccessRate {
    label: &'static str,
    success: &'static Counter,
    failure: &'static Counter,
    threshold: f64,
}

impl SuccessRate {
    pub const fn new(label: &'static str, success: &'static Counter, failure: &'static Counter, threshold: f64) -> Self {
        SuccessRate {
            label,
            success,
            failure,
            threshold,
        }
    }

    fn evaluate(&self, previous_state: &mut SuccessRateState) -> Condition {
        let current_state = SuccessRateState {
            success: self.success.read(),
            failure: self.failure.read(),
        };
        if current_state.success < previous_state.success {
            *previous_state = current_state;
            return Condition::Firing;
        }
        if current_state.success - previous_state.success >= (1u64<<56) {
            *previous_state = current_state;
            return Condition::Firing;
        }
        if current_state.failure < previous_state.failure {
            *previous_state = current_state;
            return Condition::Firing;
        }
        if current_state.failure - previous_state.failure >= (1u64<<56)  {
            *previous_state = current_state;
            return Condition::Firing;
        }
        let success = (current_state.success - previous_state.success) as f64;
        let failure = (current_state.failure - previous_state.failure) as f64;
        if success + failure <= 0.0 {
            *previous_state = current_state;
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

/////////////////////////////////////////// FireWhenClick //////////////////////////////////////////

#[derive(Clone,Debug,Default)]
struct FireWhenClickState {
    count: u64,
}

pub struct FireWhenClick {
    label: &'static str,
    counter: &'static Counter,
}

impl FireWhenClick {
    pub const fn new(label: &'static str, counter: &'static Counter) -> Self {
        Self {
            label,
            counter
        }
    }

    fn evaluate(&self, previous_state: &mut FireWhenClickState) -> Condition {
        let current_state = FireWhenClickState {
            count: self.counter.read(),
        };
        if current_state.count < previous_state.count {
            *previous_state = current_state;
            return Condition::Firing;
        }
        if current_state.count > previous_state.count {
            *previous_state = current_state;
            return Condition::Firing;
        }
        *previous_state = current_state;
        Condition::Stable
    }
}

///////////////////////////////////////// FiringSuccessRate ////////////////////////////////////////

struct FiringSuccessRate {
    record: Vec<(Condition, SuccessRateState)>,
}

impl FiringSuccessRate {
    fn new() -> Self {
        Self {
            record: Vec::new(),
        }
    }

    fn report_stable(&mut self, state: &SuccessRateState) {
        self.record.push((Condition::Stable, state.clone()));
    }

    fn report_firing(&mut self, state: &SuccessRateState) {
        self.record.push((Condition::Firing, state.clone()));
    }
}

//////////////////////////////////////// FiringFireWhenClick ///////////////////////////////////////

struct FiringFireWhenClick {
    record: Vec<(Condition, FireWhenClickState)>,
}

impl FiringFireWhenClick {
    fn new() -> Self {
        Self {
            record: Vec::new(),
        }
    }

    fn report_stable(&mut self, state: &FireWhenClickState) {
        self.record.push((Condition::Stable, state.clone()));
    }

    fn report_firing(&mut self, state: &FireWhenClickState) {
        self.record.push((Condition::Firing, state.clone()));
    }
}

///////////////////////////////////////////// HeyListen ////////////////////////////////////////////

pub struct HeyListen {
    success_rates: Vec<(&'static SuccessRate, SuccessRateState)>,
    success_rates_firing: HashMap<&'static str, FiringSuccessRate>,
    fire_when_clicks: Vec<(&'static FireWhenClick, FireWhenClickState)>,
    fire_when_clicks_firing: HashMap<&'static str, FiringFireWhenClick>,
}

impl HeyListen {
    pub fn new() -> Self {
        Self {
            success_rates: Vec::new(),
            success_rates_firing: HashMap::new(),
            fire_when_clicks: Vec::new(),
            fire_when_clicks_firing: HashMap::new(),
        }
    }

    pub fn register_success_rate(&mut self, alert: &'static SuccessRate) {
        self.success_rates.push((alert, SuccessRateState::default()));
    }

    pub fn register_fire_when_click(&mut self, alert: &'static FireWhenClick) {
        self.fire_when_clicks.push((alert, FireWhenClickState::default()));
    }

    pub fn evaluate(&mut self) {
        for (success_rate, ref mut success_rate_state) in self.success_rates.iter_mut() {
            let firing = self.success_rates_firing.get_mut(success_rate.label);
            let report = success_rate.evaluate(success_rate_state);
            match (firing, report) {
                (Some(ref mut firing), Condition::Firing) => {
                    firing.report_firing(success_rate_state);
                },
                (Some(ref mut firing), Condition::Stable) => {
                    firing.report_stable(success_rate_state);
                },
                (None, Condition::Firing) => {
                    let mut firing = FiringSuccessRate::new();
                    firing.report_firing(success_rate_state);
                    self.success_rates_firing.insert(success_rate.label, firing);
                }
                (None, Condition::Stable) => { /* intentonally blank */ }
            }
        }

        for (fire_when_click, ref mut fire_when_click_state) in self.fire_when_clicks.iter_mut() {
            let firing = self.fire_when_clicks_firing.get_mut(fire_when_click.label);
            let report = fire_when_click.evaluate(fire_when_click_state);
            match (firing, report) {
                (Some(ref mut firing), Condition::Firing) => {
                    firing.report_firing(fire_when_click_state);
                },
                (Some(ref mut firing), Condition::Stable) => {
                    firing.report_stable(fire_when_click_state);
                },
                (None, Condition::Firing) => {
                    let mut firing = FiringFireWhenClick::new();
                    firing.report_firing(fire_when_click_state);
                    self.fire_when_clicks_firing.insert(fire_when_click.label, firing);
                },
                (None, Condition::Stable) => { /* intentionally blank */ },
            }
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_rate() {
        static COUNTER1: Counter = Counter::new("success.rate.counter.1");
        static COUNTER2: Counter = Counter::new("success.rate.counter.2");
        static SUCCESS: SuccessRate = SuccessRate::new("success.rate.test", &COUNTER1, &COUNTER2, 0.999);

        let mut state = SuccessRateState::default();

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

    #[test]
    fn fire_when_click() {
        static COUNTER: Counter = Counter::new("fire.when.click.counter");
        static FIRE_WHEN_CLICK: FireWhenClick = FireWhenClick::new("fire.when.click.alert", &COUNTER);

        let mut state = FireWhenClickState::default();

        assert_eq!(Condition::Stable, FIRE_WHEN_CLICK.evaluate(&mut state));

        COUNTER.click();
        assert_eq!(Condition::Firing, FIRE_WHEN_CLICK.evaluate(&mut state));
        assert_eq!(Condition::Stable, FIRE_WHEN_CLICK.evaluate(&mut state));
    }
}
