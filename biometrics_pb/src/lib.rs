//! biometrics_pb provides protocol buffers corresponding to biometric readings.

use std::fmt::{Display, Formatter};

use one_two_eight::{generate_id, generate_id_prototk};

use prototk_derive::Message;

///////////////////////////////////////////// SensorID /////////////////////////////////////////////

generate_id! {SensorID, "sensor:"}
generate_id_prototk! {SensorID}

/////////////////////////////////////////// SensorReading //////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub enum SensorReading {
    #[default]
    #[prototk(1, message)]
    Zero,
    #[prototk(2, message)]
    Counter(CounterPb),
    #[prototk(3, message)]
    Gauge(GaugePb),
    #[prototk(4, message)]
    Moments(MomentsPb),
}

impl Display for SensorReading {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            SensorReading::Zero => write!(f, "_"),
            SensorReading::Counter(counter) => {
                write!(f, "{}", counter.count)
            },
            SensorReading::Gauge(gauge) => {
                write!(f, "{}", gauge.value)
            },
            SensorReading::Moments(moments) => {
                write!(f, "{}, ...", moments.n)
            },
        }
    }
}

////////////////////////////////////////////// Counter /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct CounterPb {
    #[prototk(1, uint64)]
    pub count: u64,
}

impl Display for CounterPb {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "Counter({})", self.count)
    }
}

impl From<u64> for CounterPb {
    fn from(count: u64) -> Self {
        Self { count }
    }
}

/////////////////////////////////////////////// Gauge //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct GaugePb {
    #[prototk(2, double)]
    pub value: f64,
}

impl Display for GaugePb {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "Gauge({})", self.value)
    }
}

impl From<f64> for GaugePb {
    fn from(value: f64) -> Self {
        Self { value }
    }
}

////////////////////////////////////////////// Moments /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct MomentsPb {
    #[prototk(3, uint64)]
    pub n: u64,
    #[prototk(4, double)]
    pub m1: f64,
    #[prototk(5, double)]
    pub m2: f64,
    #[prototk(6, double)]
    pub m3: f64,
    #[prototk(7, double)]
    pub m4: f64,
}

impl MomentsPb {
    pub const fn new(n: u64, m1: f64, m2: f64, m3: f64, m4: f64) -> Self {
        Self {
            n,
            m1,
            m2,
            m3,
            m4,
        }
    }

    pub fn into_moments(self) -> biometrics::moments::Moments {
        biometrics::moments::Moments {
            n: self.n,
            m1: self.m1,
            m2: self.m2,
            m3: self.m3,
            m4: self.m4,
        }
    }
}

impl Display for MomentsPb {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "Moments(n={}, ...)", self.n)
    }
}

impl From<biometrics::moments::Moments> for MomentsPb {
    fn from(m: biometrics::moments::Moments) -> Self {
        Self {
            n: m.n,
            m1: m.m1,
            m2: m.m2,
            m3: m.m3,
            m4: m.m4,
        }
    }
}
