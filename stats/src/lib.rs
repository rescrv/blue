use std::fmt::Debug;

pub mod sensors;
pub mod moments;

pub use sensors::Counter;
pub use sensors::Gauge;
pub use sensors::Moments;

////////////////////////////////////////////// Sensor //////////////////////////////////////////////

pub trait Sensor: Debug + Send {
}
