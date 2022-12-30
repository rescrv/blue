use std::fmt::Display;

use biometrics::{click, Counter, Gauge, Sensor};

use clue::Trace;

use id::generate_id;

use prototk::field_types::*;

///////////////////////////////////////////// FiringID /////////////////////////////////////////////

generate_id!{FiringID, "firing:"}

///////////////////////////////////////////// Condition ////////////////////////////////////////////

#[derive(Clone, Debug)]
pub enum Condition {
    Stable {
        context: String,
    },
    Firing {
        description: &'static str,
        context: String,
    },
}

impl Condition {
    pub fn stable() -> Self {
        click!("hey_listen.condition.stable");
        Condition::Stable {
            context: String::default(),
        }
    }

    pub fn firing(description: &'static str) -> Self {
        click!("hey_listen.condition.firing");
        Condition::Firing {
            description,
            context: String::default(),
        }
    }

    pub fn with_context<V: Display>(mut self, field_name: &str, field_value: V) -> Self {
        let ctx = &format!("{} = {}\n", field_name, field_value);
        match &mut self {
            Condition::Stable { context } => context.push_str(ctx),
            Condition::Firing { description: _, context } => context.push_str(ctx),
        };
        self
    }

    pub fn description(&self) -> &str {
        match &self {
            Condition::Stable { context: _ } => "success",
            Condition::Firing { description, context: _ } => description,
        }
    }

    pub fn context(&self) -> &str {
        match &self {
            Condition::Stable { context } => context,
            Condition::Firing { description: _, context } => context,
        }
    }
}

////////////////////////////////////////////// Monitor /////////////////////////////////////////////

trait Monitor {
    type State: Default;

    fn label(&self) -> &'static str;
    fn witness(&self) -> Self::State;
    fn evaluate(&self, previous: &Self::State) -> Condition;
}

impl<M: Monitor> Monitor for &M {
    type State = M::State;

    fn label(&self) -> &'static str {
        M::label(self)
    }

    fn witness(&self) -> Self::State {
        M::witness(self)
    }

    fn evaluate(&self, previous: &Self::State) -> Condition {
        M::evaluate(self, previous)
    }
}

////////////////////////////////////////// StickyCondition /////////////////////////////////////////

struct StickyCondition {
    firing: FiringID,
    sticky: Condition,
    recent: Option<Condition>,
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

struct State<M: Monitor + 'static> {
    monitor: &'static M,
    state: M::State,
    sticky: Option<StickyCondition>,
}

impl<M: Monitor + 'static> State<M> {
    pub fn new(monitor: &'static M) -> Self {
        Self {
            monitor,
            state: M::State::default(),
            sticky: None,
        }
    }

    fn evaluate(&mut self) -> Condition {
        let condition = self.monitor.evaluate(&mut self.state);
        self.state = self.monitor.witness();
        if let Condition::Firing { description, context } = &condition {
            if self.sticky.is_none() {
                let firing = FiringID::generate().unwrap_or(FiringID::default());
                self.sticky = Some(StickyCondition {
                    firing: firing.clone(),
                    sticky: condition.clone(),
                    recent: None,
                });
                Trace::new("hey_listen.make_sticky")
                    .with_context::<stringref>("label", 1, self.monitor.label())
                    .with_context::<string>("firing", 2, firing.human_readable())
                    .with_context::<stringref>("description", 3, description)
                    .with_context::<stringref>("context", 4, &context)
                    .finish();
            }
        }
        if let Some(sticky) = &mut self.sticky {
            Trace::new("hey_listen.recent")
                .with_context::<stringref>("label", 1, self.monitor.label())
                .with_context::<string>("firing", 2, sticky.firing.human_readable())
                .with_context::<stringref>("description", 3, condition.description())
                .with_context::<stringref>("context", 4, condition.context())
                .finish();
            sticky.recent = Some(condition.clone());
        }
        condition
    }

    fn reset(&mut self, firing: FiringID) -> bool {
        match &self.sticky {
            Some(sticky) if sticky.firing == firing => {
                Trace::new("hey_listen.reset")
                    .with_context::<stringref>("label", 1, self.monitor.label())
                    .with_context::<stringref>("firing", 2, &firing.human_readable())
                    .finish();
                self.sticky = None;
                true
            },
            Some(sticky) => {
                Trace::new("hey_listen.reset.wrong_id")
                    .with_context::<stringref>("label", 1, self.monitor.label())
                    .with_context::<stringref>("expected", 2, &sticky.firing.human_readable())
                    .with_context::<stringref>("provided", 3, &firing.human_readable())
                    .finish();
                false
            },
            None => {
                Trace::new("hey_listen.reset.nothing_to_reset")
                    .with_context::<stringref>("label", 1, self.monitor.label())
                    .with_context::<stringref>("provided", 2, &firing.human_readable());
                false
            },
        }
    }
}

///////////////////////////////////////////// HeyListen ////////////////////////////////////////////

pub struct HeyListen {
    success_rate: Vec<State<SuccessRate>>,
    stationary: Vec<State<Stationary>>,
    below_threshold: Vec<State<BelowThreshold>>,
    above_threshold: Vec<State<AboveThreshold>>,
}

impl HeyListen {
    pub fn new() -> Self {
        Self {
            success_rate: Vec::new(),
            stationary: Vec::new(),
            below_threshold: Vec::new(),
            above_threshold: Vec::new(),
        }
    }

    pub fn register_success_rate(&mut self, monitor: &'static SuccessRate) {
        self.success_rate.push(State::new(monitor));
    }

    pub fn register_stationary(&mut self, monitor: &'static Stationary) {
        self.stationary.push(State::new(monitor));
    }

    pub fn register_below_threshold(&mut self, monitor: &'static BelowThreshold) {
        self.below_threshold.push(State::new(monitor));
    }

    pub fn register_above_threshold(&mut self, monitor: &'static AboveThreshold) {
        self.above_threshold.push(State::new(monitor));
    }

    pub fn evaluate(&mut self) {
        for state in self.success_rate.iter_mut() {
            state.evaluate();
        }
        for state in self.stationary.iter_mut() {
            state.evaluate();
        }
        for state in self.below_threshold.iter_mut() {
            state.evaluate();
        }
        for state in self.above_threshold.iter_mut() {
            state.evaluate();
        }
    }

    pub fn firing(&self) -> impl Iterator<Item=(&'static str, FiringID, Condition, Condition)> {
        let mut iter = Vec::new();
        self.firing_one(&self.success_rate, &mut iter);
        self.firing_one(&self.stationary, &mut iter);
        self.firing_one(&self.below_threshold, &mut iter);
        self.firing_one(&self.above_threshold, &mut iter);
        iter.into_iter()
    }

    fn firing_one<M: Monitor>(&self, v: &Vec<State<M>>, out: &mut Vec<(&'static str, FiringID, Condition, Condition)>) {
        for state in v.iter() {
            if let Some(sticky) = &state.sticky {
                let label = state.monitor.label();
                let firing = sticky.firing;
                let initial = sticky.sticky.clone();
                let recent = match &sticky.recent {
                    Some(recent) => recent.clone(),
                    None => initial.clone(),
                };
                out.push((label, firing, initial, recent));
            }
        }
    }

    pub fn reset(&mut self, label: &'static str, firing: FiringID) {
        for state in self.success_rate.iter_mut() {
            if state.monitor.label() == label {
                state.reset(firing);
            }
        }
        for state in self.stationary.iter_mut() {
            if state.monitor.label() == label {
                state.reset(firing);
            }
        }
        for state in self.below_threshold.iter_mut() {
            if state.monitor.label() == label {
                state.reset(firing);
            }
        }
        for state in self.above_threshold.iter_mut() {
            if state.monitor.label() == label {
                state.reset(firing);
            }
        }
    }
}

//////////////////////////////////////////// SuccessRate ///////////////////////////////////////////

#[derive(Default)]
struct SuccessRateState {
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
    pub const fn new(
        label: &'static str,
        success: &'static Counter,
        failure: &'static Counter,
        threshold: f64,
    ) -> Self {
        Self {
            label,
            success,
            failure,
            threshold,
        }
    }
}

impl Monitor for SuccessRate {
    type State = SuccessRateState;

    fn label(&self) -> &'static str {
        self.label
    }

    fn witness(&self) -> Self::State {
        let success: u64 = self.success.read();
        let failure: u64 = self.failure.read();
        Self::State {
            success,
            failure,
        }
    }

    fn evaluate(&self, previous: &Self::State) -> Condition {
        const F64_PRECISION: u64 = 1u64 << 56;
        let current = self.witness();

        // If the succes counter regressed, witness it.
        if current.success < previous.success {
            return Condition::firing("success counter regressed")
                .with_context("current.success", current.success)
                .with_context("previous.success", previous.success);
        }
        // If the success counter difference exceeds the precision of f64.
        let success_delta: u64 = current.success - previous.success;
        if success_delta >= F64_PRECISION {
            return Condition::firing("success delta too large")
                .with_context("delta", success_delta)
                .with_context("current.success", current.success)
                .with_context("previous.success", previous.success);
        }
        // If the failure counter regressed, witness it.
        if current.failure < previous.failure {
            return Condition::firing("failure counter regressed")
                .with_context("current.failure", current.failure)
                .with_context("previous.failure", previous.failure);
        }
        // If the failure counter difference exceeds the precision of f64.
        let failure_delta: u64 = current.failure - previous.failure;
        if failure_delta >= F64_PRECISION {
            return Condition::firing("failure delta too large")
                .with_context("delta", failure_delta)
                .with_context("current.failure", current.failure)
                .with_context("previous.failure", previous.failure);
        }
        // If the total of success_delta + failure_delta exceeds the precision of f64.
        if success_delta + failure_delta >= F64_PRECISION {
            return Condition::firing("total delta too large")
                .with_context("success_delta", success_delta)
                .with_context("failure_delta", failure_delta);
        }
        // Start working with f64.
        let success = success_delta as f64;
        let failure = failure_delta as f64;
        // Check for divide by zero.
        let total = success + failure;
        if total <= 0.0 {
            return Condition::firing("total not positive")
                .with_context("success", success)
                .with_context("failure", failure);
        }
        // Compute the success rate.
        let success_rate = success / total;
        if success_rate < self.threshold {
            return Condition::firing("success rate below threshold")
                .with_context("success_rate", success_rate)
                .with_context("threshold", self.threshold);
        }
        // Return success.
        Condition::stable()
            .with_context("success_rate", success_rate)
            .with_context("threshold", self.threshold)
    }
}

//////////////////////////////////////////// Stationary ////////////////////////////////////////////

#[derive(Default)]
struct StationaryState {
    count: u64,
}

pub struct Stationary {
    label: &'static str,
    counter: &'static Counter,
}

impl Stationary {
    pub const fn new(
        label: &'static str,
        counter: &'static Counter,
    ) -> Self {
        Self {
            label,
            counter,
        }
    }
}

impl Monitor for Stationary {
    type State = StationaryState;

    fn label(&self) -> &'static str {
        self.label
    }

    fn witness(&self) -> Self::State {
        let count: u64 = self.counter.read();
        Self::State {
            count,
        }
    }

    fn evaluate(&self, previous: &Self::State) -> Condition {
        let current = self.witness();

        // If the counter regressed.
        if current.count < previous.count {
            return Condition::firing("counter regressed")
                .with_context("current", current.count)
                .with_context("previous", previous.count);
        }
        // If the counter clicked.
        if current.count > previous.count {
            return Condition::firing("counter clicked")
                .with_context("current", current.count)
                .with_context("previous", previous.count);
        }
        // Return success.
        Condition::stable()
            .with_context("count", current.count)
    }
}

////////////////////////////////////////// BelowThreshold //////////////////////////////////////////

pub struct BelowThreshold {
    label: &'static str,
    gauge: &'static Gauge,
    threshold: f64,
}

impl BelowThreshold {
    pub const fn new(
        label: &'static str,
        gauge: &'static Gauge,
        threshold: f64,
    ) -> Self {
        Self {
            label,
            gauge,
            threshold,
        }
    }
}

impl Monitor for BelowThreshold {
    type State = ();

    fn label(&self) -> &'static str {
        self.label
    }

    fn witness(&self) -> Self::State {
        ()
    }

    fn evaluate(&self, _: &Self::State) -> Condition {
        let current: f64 = self.gauge.read();
        if current < self.threshold {
            return Condition::stable()
                .with_context("current", current);
        } else {
            return Condition::firing("value exceeds threshold")
                .with_context("current", current)
                .with_context("threshold", self.threshold);
        }
    }
}

////////////////////////////////////////// AboveThreshold //////////////////////////////////////////

pub struct AboveThreshold {
    label: &'static str,
    gauge: &'static Gauge,
    threshold: f64,
}

impl AboveThreshold {
    pub const fn new(
        label: &'static str,
        gauge: &'static Gauge,
        threshold: f64,
    ) -> Self {
        Self {
            label,
            gauge,
            threshold,
        }
    }
}

impl Monitor for AboveThreshold {
    type State = ();

    fn label(&self) -> &'static str {
        self.label
    }

    fn witness(&self) -> Self::State {
        ()
    }

    fn evaluate(&self, _: &Self::State) -> Condition {
        let current: f64 = self.gauge.read();
        if current > self.threshold {
            return Condition::stable()
                .with_context("current", current);
        } else {
            return Condition::firing("value below threshold")
                .with_context("current", current)
                .with_context("threshold", self.threshold);
        }
    }
}
