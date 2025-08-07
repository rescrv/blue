#![doc = include_str!("../README.md")]

use std::fmt::Display;

use biometrics::{Counter, Gauge, Sensor};

use one_two_eight::generate_id;

///////////////////////////////////////////// FiringID /////////////////////////////////////////////

generate_id! {FiringID, "firing:"}

///////////////////////////////////////////// Condition ////////////////////////////////////////////

/// A condition represents the state of a monitor.
#[derive(Clone, Debug)]
pub enum Condition {
    /// When the system is stable, it is represented by this variant.
    Stable {
        /// The context associated with this condition.
        context: String,
    },
    /// When the system witnesses an event, it is represented by this variant.
    Firing {
        /// The description of what caused it to fire.
        description: &'static str,
        /// The context associated with this condition.
        context: String,
    },
}

impl Condition {
    /// Returns a new stable condition.
    pub fn stable() -> Self {
        // TODO(rescrv): click!("hey_listen.condition.stable");
        Condition::Stable {
            context: String::default(),
        }
    }

    /// Returns a firing condition.
    pub fn firing(description: &'static str) -> Self {
        // TODO(rescrv): click!("hey_listen.condition.firing");
        Condition::Firing {
            description,
            context: String::default(),
        }
    }

    /// Adds context to the condition.
    pub fn with_context<V: Display>(mut self, field_name: &str, field_value: V) -> Self {
        let ctx = &format!("{field_name} = {field_value}\n");
        match &mut self {
            Condition::Stable { context } => context.push_str(ctx),
            Condition::Firing {
                description: _,
                context,
            } => context.push_str(ctx),
        };
        self
    }

    /// Returns a description of the condition.
    pub fn description(&self) -> &str {
        match &self {
            Condition::Stable { context: _ } => "success",
            Condition::Firing {
                description,
                context: _,
            } => description,
        }
    }

    /// Returns the context associated with this condition.
    pub fn context(&self) -> &str {
        match &self {
            Condition::Stable { context } => context,
            Condition::Firing {
                description: _,
                context,
            } => context,
        }
    }
}

////////////////////////////////////////////// Monitor /////////////////////////////////////////////

/// Monitors the state of the process.
trait Monitor {
    /// The type of the state that's used to carry state between successive calls to [evaluate].
    type State: Default;

    /// The label associated with the monitor.
    fn label(&self) -> &'static str;
    /// Witness the state of the monitor.
    fn witness(&self) -> Self::State;
    /// Evaluate the condition monitored by the monitor.
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
        let condition = self.monitor.evaluate(&self.state);
        self.state = self.monitor.witness();
        if let Condition::Firing {
            description: _,
            context: _,
        } = &condition
        {
            if self.sticky.is_none() {
                let firing = FiringID::generate().unwrap_or_default();
                self.sticky = Some(StickyCondition {
                    firing,
                    sticky: condition.clone(),
                    recent: None,
                });
            }
        }
        if let Some(sticky) = &mut self.sticky {
            sticky.recent = Some(condition.clone());
        }
        condition
    }

    fn reset(&mut self, firing: FiringID) -> bool {
        match &self.sticky {
            Some(sticky) if sticky.firing == firing => {
                self.sticky = None;
                true
            }
            Some(_) => false,
            None => false,
        }
    }
}

///////////////////////////////////////////// HeyListen ////////////////////////////////////////////

/// HeyListen watches a set of monitors in a way that allows conditions to be reliably detected.
/// It is intended that there will be a `register_tatl` function or similar in each module with a
/// monitor, and the [HeyListen] instance passed to that function will watch the monitors.
pub struct HeyListen {
    success_rate: Vec<State<SuccessRate>>,
    stationary: Vec<State<Stationary>>,
    below_threshold: Vec<State<BelowThreshold>>,
    above_threshold: Vec<State<AboveThreshold>>,
}

impl HeyListen {
    /// Create a new instance of HeyListen.
    pub fn new() -> Self {
        Self {
            success_rate: Vec::new(),
            stationary: Vec::new(),
            below_threshold: Vec::new(),
            above_threshold: Vec::new(),
        }
    }

    /// Register a success rate that computes the ratio of success to total activity.
    pub fn register_success_rate(&mut self, monitor: &'static SuccessRate) {
        self.success_rate.push(State::new(monitor));
    }

    /// Register a stationary that triggers when the value changes.
    pub fn register_stationary(&mut self, monitor: &'static Stationary) {
        self.stationary.push(State::new(monitor));
    }

    /// Register a monitor that detects when something is below the provided threshold.
    pub fn register_below_threshold(&mut self, monitor: &'static BelowThreshold) {
        self.below_threshold.push(State::new(monitor));
    }

    /// Register a monitor that detects when something is above the provided threshold.
    pub fn register_above_threshold(&mut self, monitor: &'static AboveThreshold) {
        self.above_threshold.push(State::new(monitor));
    }

    /// Evaluate the registered monitors.
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

    /// Return the list of firing conditions.
    ///
    /// The return FiringID is stable until a call to reset with the label and FiringID.
    pub fn firing(&self) -> impl Iterator<Item = (&'static str, FiringID, Condition, Condition)> {
        let mut iter = Vec::new();
        self.firing_one(&self.success_rate, &mut iter);
        self.firing_one(&self.stationary, &mut iter);
        self.firing_one(&self.below_threshold, &mut iter);
        self.firing_one(&self.above_threshold, &mut iter);
        iter.into_iter()
    }

    fn firing_one<M: Monitor>(
        &self,
        v: &[State<M>],
        out: &mut Vec<(&'static str, FiringID, Condition, Condition)>,
    ) {
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

    /// Reset the specified monitor.  This is used to acknowledge alerts.
    pub fn reset(&mut self, label: &str, firing: FiringID) {
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

impl Default for HeyListen {
    fn default() -> Self {
        Self::new()
    }
}

//////////////////////////////////////////// SuccessRate ///////////////////////////////////////////

#[derive(Default)]
struct SuccessRateState {
    success: u64,
    failure: u64,
}

/// A SuccessRate compares success/(success + failure) from two counters.
pub struct SuccessRate {
    label: &'static str,
    success: &'static Counter,
    failure: &'static Counter,
    threshold: f64,
}

impl SuccessRate {
    /// Create a new success rate.
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
        Self::State { success, failure }
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

/// A Stationary triggers whenever a counter increments.
pub struct Stationary {
    label: &'static str,
    counter: &'static Counter,
}

impl Stationary {
    /// Create a new stationary that watches the specified counter.
    pub const fn new(label: &'static str, counter: &'static Counter) -> Self {
        Self { label, counter }
    }
}

impl Monitor for Stationary {
    type State = StationaryState;

    fn label(&self) -> &'static str {
        self.label
    }

    fn witness(&self) -> Self::State {
        let count: u64 = self.counter.read();
        Self::State { count }
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
        Condition::stable().with_context("count", current.count)
    }
}

////////////////////////////////////////// BelowThreshold //////////////////////////////////////////

/// BelowThreshold monitors a gauge to see if it dips below a threshold.
pub struct BelowThreshold {
    label: &'static str,
    gauge: &'static Gauge,
    threshold: f64,
}

impl BelowThreshold {
    /// Create a new BelowTheshold monitor.
    pub const fn new(label: &'static str, gauge: &'static Gauge, threshold: f64) -> Self {
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

    fn witness(&self) -> Self::State {}

    fn evaluate(&self, _: &Self::State) -> Condition {
        let current: f64 = self.gauge.read();
        if current < self.threshold {
            Condition::stable().with_context("current", current)
        } else {
            Condition::firing("value exceeds threshold")
                .with_context("current", current)
                .with_context("threshold", self.threshold)
        }
    }
}

////////////////////////////////////////// AboveThreshold //////////////////////////////////////////

/// AboveThreshold monitors a gauge to see if it dips below a threshold.
pub struct AboveThreshold {
    label: &'static str,
    gauge: &'static Gauge,
    threshold: f64,
}

impl AboveThreshold {
    /// Creates a new AboveThreshold.
    pub const fn new(label: &'static str, gauge: &'static Gauge, threshold: f64) -> Self {
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

    fn witness(&self) -> Self::State {}

    fn evaluate(&self, _: &Self::State) -> Condition {
        let current: f64 = self.gauge.read();
        if current > self.threshold {
            Condition::stable().with_context("current", current)
        } else {
            Condition::firing("value below threshold")
                .with_context("current", current)
                .with_context("threshold", self.threshold)
        }
    }
}
