use std::collections::hash_map::{Entry, HashMap};
use std::collections::hash_set::HashSet;
use std::fmt::Debug;
use std::time::SystemTime;

use rand::{thread_rng, RngCore};

use biometrics::Counter;

///////////////////////////////////////////// Condition ////////////////////////////////////////////

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Condition {
    Firing,
    Stable,
}

impl Default for Condition {
    fn default() -> Self {
        Condition::Firing
    }
}

/////////////////////////////////////////////// Alert //////////////////////////////////////////////

trait Alert: Clone {
    type State: Clone + Debug + Default;
    type Delta: Clone + Debug + Default;

    fn label(&self) -> &'static str;
    fn observations_to_keep(&self) -> usize;
    fn evaluate(&self, previous_state: &Self::State) -> (Condition, Self::State, Self::Delta);

    fn print(state: Self::State, delta: Self::Delta) -> String;
}

//////////////////////////////////////////// Observation ///////////////////////////////////////////

#[derive(Clone, Debug)]
struct Observation<A: Alert> {
    iter: u64,
    when: SystemTime,
    state: A::State,
    delta: A::Delta,
}

/////////////////////////////////////////// AlertSampler ///////////////////////////////////////////

#[derive(Clone, Debug)]
struct AlertSampler<A: Alert> {
    prev: A::State,
    iter: u64,
    samples: Vec<Observation<A>>,
}

impl<A: Alert> AlertSampler<A> {
    fn observe(&mut self, observations_to_keep: usize, state: A::State, delta: A::Delta) {
        self.iter += 1;
        let iter = self.iter;
        let when = SystemTime::now();
        let obs = Observation {
            iter,
            when,
            state,
            delta,
        };
        if self.samples.len() >= observations_to_keep {
            let idx = (thread_rng().next_u64() as usize) % observations_to_keep;
            self.samples[idx] = obs;
            self.samples.truncate(observations_to_keep);
        } else {
            self.samples.push(obs);
        }
    }

    fn clear(&mut self, iter: u64) -> bool {
        for idx in 0..self.samples.len() {
            if self.samples[idx].iter == iter {
                self.samples[idx] = self.samples[self.samples.len() - 1].clone();
                self.samples.pop();
                return true;
            }
        }
        return false;
    }

    fn clear_all(&mut self) {
        self.samples.truncate(0);
    }
}

impl<A: Alert> Default for AlertSampler<A> {
    fn default() -> Self {
        Self {
            prev: A::State::default(),
            iter: 0,
            samples: Vec::new(),
        }
    }
}

/////////////////////////////////////////// AlertRegistry //////////////////////////////////////////

#[derive(Clone, Debug)]
struct AlertRegistry<A: Alert + 'static> {
    registered: HashMap<&'static str, (&'static A, AlertSampler<A>)>,
}

impl<A: Alert + 'static> AlertRegistry<A> {
    fn register(&mut self, alert: &'static A) {
        self.registered
            .insert(alert.label(), (alert, AlertSampler::default()));
    }

    fn evaluate(&mut self) {
        for (_, (alert, sampler)) in self.registered.iter_mut() {
            let (condition, new_state, delta) = alert.evaluate(&sampler.prev);
            if condition == Condition::Firing {
                sampler.observe(alert.observations_to_keep(), new_state, delta);
            }
        }
    }

    fn clear(&mut self, label: &'static str, iter: u64) -> bool {
        if let Entry::Occupied(entry) = self.registered.entry(label) {
            let (_, sampler) = entry.into_mut();
            sampler.clear(iter)
        } else {
            false
        }
    }

    fn clear_all(&mut self, label: &'static str) {
        if let Entry::Occupied(entry) = self.registered.entry(label) {
            let (_, sampler) = entry.into_mut();
            sampler.clear_all();
        }
    }
}

impl<A: Alert + 'static> Default for AlertRegistry<A> {
    fn default() -> Self {
        Self {
            registered: HashMap::default(),
        }
    }
}

//////////////////////////////////////////// SuccessRate ///////////////////////////////////////////

#[derive(Clone, Debug, Default)]
struct SuccessRateState {
    success: u64,
    failure: u64,
}

#[derive(Clone, Debug, Default)]
struct SuccessRateDelta {
    success: u64,
    failure: u64,
}

#[derive(Clone, Debug)]
pub struct SuccessRate {
    label: &'static str,
    success: &'static Counter,
    failure: &'static Counter,
    threshold: f64,
    observations_to_keep: usize,
}

impl SuccessRate {
    pub const fn new(
        label: &'static str,
        success: &'static Counter,
        failure: &'static Counter,
        threshold: f64,
        observations_to_keep: usize,
    ) -> Self {
        SuccessRate {
            label,
            success,
            failure,
            threshold,
            observations_to_keep,
        }
    }
}

impl Alert for SuccessRate {
    type State = SuccessRateState;
    type Delta = SuccessRateDelta;

    fn label(&self) -> &'static str {
        self.label
    }

    fn observations_to_keep(&self) -> usize {
        self.observations_to_keep
    }

    fn evaluate(&self, previous_state: &Self::State) -> (Condition, Self::State, Self::Delta) {
        let current_state = SuccessRateState {
            success: self.success.read(),
            failure: self.failure.read(),
        };
        let delta = SuccessRateDelta {
            success: current_state.success,
            failure: current_state.failure,
        };
        if current_state.success < previous_state.success {
            return (Condition::Firing, current_state, delta);
        }
        if current_state.success - previous_state.success >= (1u64 << 56) {
            return (Condition::Firing, current_state, delta);
        }
        if current_state.failure < previous_state.failure {
            return (Condition::Firing, current_state, delta);
        }
        if current_state.failure - previous_state.failure >= (1u64 << 56) {
            return (Condition::Firing, current_state, delta);
        }
        let delta = SuccessRateDelta {
            success: current_state.success - previous_state.success,
            failure: current_state.failure - previous_state.failure,
        };
        let success = delta.success as f64;
        let failure = delta.failure as f64;
        if success + failure <= 0.0 {
            return (Condition::Firing, current_state, delta);
        }
        let success_rate = success / (success + failure);
        if success_rate >= self.threshold {
            (Condition::Stable, current_state, delta)
        } else {
            (Condition::Firing, current_state, delta)
        }
    }

    fn print(state: Self::State, delta: Self::Delta) -> String {
        format!("{} {} {} {}", state.success, state.failure, delta.success, delta.failure)
    }
}

///////////////////////////////////////////// Clicking /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
struct ClickingState {
    count: u64,
}

#[derive(Clone, Debug, Default)]
struct ClickingDelta {
    count: u64,
}

#[derive(Clone, Debug)]
pub struct Clicking {
    label: &'static str,
    counter: &'static Counter,
    observations_to_keep: usize,
}

impl Clicking {
    pub const fn new(
        label: &'static str,
        counter: &'static Counter,
        observations_to_keep: usize,
    ) -> Self {
        Clicking {
            label,
            counter,
            observations_to_keep,
        }
    }
}

impl Alert for Clicking {
    type State = ClickingState;
    type Delta = ClickingDelta;

    fn label(&self) -> &'static str {
        self.label
    }

    fn observations_to_keep(&self) -> usize {
        self.observations_to_keep
    }

    fn evaluate(&self, previous_state: &Self::State) -> (Condition, Self::State, Self::Delta) {
        let current_state = ClickingState {
            count: self.counter.read(),
        };
        let delta = ClickingDelta {
            count: current_state.count - previous_state.count,
        };
        let condition = if current_state.count == previous_state.count {
            Condition::Stable
        } else {
            Condition::Firing
        };
        (condition, current_state, delta)
    }

    fn print(state: Self::State, delta: Self::Delta) -> String {
        format!("{} {}", state.count, delta.count)
    }
}

///////////////////////////////////////////// HeyListen ////////////////////////////////////////////

#[derive(Default)]
pub struct HeyListen {
    labels: HashSet<&'static str>,
    success_rates: AlertRegistry<SuccessRate>,
    clicking: AlertRegistry<Clicking>,
}

impl HeyListen {
    pub fn register_success_rate(&mut self, alert: &'static SuccessRate) {
        self.claim_label(alert.label());
        self.success_rates.register(alert);
    }

    pub fn register_clicking(&mut self, alert: &'static Clicking) {
        self.claim_label(alert.label());
        self.clicking.register(alert);
    }

    pub fn evaluate(&mut self) {
        self.success_rates.evaluate();
        self.clicking.evaluate();
    }

    pub fn clear(&mut self, label: &'static str, iter: u64) -> bool {
        self.success_rates.clear(label, iter) || self.clicking.clear(label, iter)
    }

    pub fn clear_all(&mut self, label: &'static str) {
        self.success_rates.clear_all(label);
        self.clicking.clear_all(label);
    }

    fn claim_label(&mut self, label: &'static str) {
        if self.labels.contains(label) {
            panic!("Cannot register \"{}\" twice", label);
        }
        self.labels.insert(label);
    }
}
