//! biometrics_pb provides protocol buffers corresponding to biometric readings.

use one_two_eight::{generate_id, generate_id_prototk};

use prototk_derive::Message;

///////////////////////////////////////////// SensorID /////////////////////////////////////////////

generate_id! {SensorID, "sensor:"}
generate_id_prototk! {SensorID}

////////////////////////////////////////////// Counter /////////////////////////////////////////////

#[derive(Clone, Debug, Default, Message)]
pub struct CounterPb {
    #[prototk(1, uint64)]
    pub count: u64,
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
