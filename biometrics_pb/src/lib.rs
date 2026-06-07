#![doc = include_str!("../README.md")]

use std::fmt::{Display, Formatter};

use one_two_eight::{generate_id, generate_id_prototk};

use prototk_derive::Message;

///////////////////////////////////////////// SensorID /////////////////////////////////////////////

generate_id! {SensorID, "sensor:"}
generate_id_prototk! {SensorID}

/////////////////////////////////////////// SensorReading //////////////////////////////////////////

/// A protocol-buffer representation of one biometric sensor reading.
#[derive(Clone, Debug, Default, PartialEq, prototk_derive::Message)]
pub enum SensorReading {
    /// An unset reading.
    #[default]
    #[prototk(1, message)]
    Zero,
    /// A counter reading.
    #[prototk(2, message)]
    Counter(CounterPb),
    /// A gauge reading.
    #[prototk(3, message)]
    Gauge(GaugePb),
    /// A moments reading.
    #[prototk(4, message)]
    Moments(MomentsPb),
    /// A histogram reading.
    #[prototk(5, message)]
    Histogram(HistogramPb),
}

impl Display for SensorReading {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            SensorReading::Zero => write!(f, "_"),
            SensorReading::Counter(counter) => {
                write!(f, "{}", counter.count)
            }
            SensorReading::Gauge(gauge) => {
                write!(f, "{}", gauge.value)
            }
            SensorReading::Moments(moments) => {
                write!(f, "{}, ...", moments.n)
            }
            SensorReading::Histogram(histogram) => {
                write!(
                    f,
                    "{} sig figs, {} buckets",
                    histogram.sig_figs,
                    histogram.buckets.len()
                )
            }
        }
    }
}

////////////////////////////////////////////// Counter /////////////////////////////////////////////

/// A protocol-buffer counter reading.
#[derive(Clone, Debug, Default, Message, PartialEq)]
pub struct CounterPb {
    /// The cumulative counter value.
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

/// A protocol-buffer gauge reading.
#[derive(Clone, Debug, Default, Message, PartialEq)]
pub struct GaugePb {
    /// The latest gauge value.
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

/// A protocol-buffer moments reading.
#[derive(Clone, Debug, Default, Message, PartialEq)]
pub struct MomentsPb {
    /// The number of observations.
    #[prototk(3, uint64)]
    pub n: u64,
    /// The first moment accumulator.
    #[prototk(4, double)]
    pub m1: f64,
    /// The second moment accumulator.
    #[prototk(5, double)]
    pub m2: f64,
    /// The third moment accumulator.
    #[prototk(6, double)]
    pub m3: f64,
    /// The fourth moment accumulator.
    #[prototk(7, double)]
    pub m4: f64,
}

impl MomentsPb {
    /// Create a new moments protocol-buffer reading.
    pub const fn new(n: u64, m1: f64, m2: f64, m3: f64, m4: f64) -> Self {
        Self { n, m1, m2, m3, m4 }
    }

    /// Convert this reading into the biometrics moments type.
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

///////////////////////////////////////////// Histogram ////////////////////////////////////////////

/// A protocol-buffer histogram reading.
#[derive(Clone, Debug, Default, Message, PartialEq)]
pub struct HistogramPb {
    /// The number of significant figures used by the histogram.
    #[prototk(8, uint32)]
    pub sig_figs: u32,
    /// Bucket counts indexed by [`sig_fig_histogram::SigFigBucketizer`].
    #[prototk(9, uint64)]
    pub buckets: Vec<u64>,
}

impl HistogramPb {
    /// Create a new histogram protocol-buffer reading.
    pub fn new(sig_figs: u32, buckets: Vec<u64>) -> Self {
        Self { sig_figs, buckets }
    }

    /// Convert this reading into the biometrics histogram type.
    pub fn try_into_histogram(self) -> Result<sig_fig_histogram::Histogram, &'static str> {
        let sig_figs: i32 = self
            .sig_figs
            .try_into()
            .map_err(|_| "sig figs out of bounds")?;
        if !(1..=4).contains(&sig_figs) {
            return Err("sig figs out of bounds");
        }
        let bucketizer = sig_fig_histogram::SigFigBucketizer::new(sig_figs);
        let mut histogram = sig_fig_histogram::Histogram::new(sig_figs);
        for (idx, count) in self.buckets.into_iter().enumerate() {
            if count == 0 {
                continue;
            }
            histogram
                .observe_n(bucketizer.boundary_for(idx as i32), count)
                .map_err(|_| "could not observe histogram bucket")?;
        }
        Ok(histogram)
    }
}

impl Display for HistogramPb {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Histogram(sig_figs={}, buckets={})",
            self.sig_figs,
            self.buckets.len()
        )
    }
}

impl From<sig_fig_histogram::Histogram> for HistogramPb {
    fn from(histogram: sig_fig_histogram::Histogram) -> Self {
        let mut buckets = histogram
            .iter()
            .map(|(_, count)| count)
            .collect::<Vec<u64>>();
        while buckets.last() == Some(&0) {
            buckets.pop();
        }
        Self {
            sig_figs: histogram.sig_figs() as u32,
            buckets,
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use buffertk::{Unpacker, stack_pack};

    use super::*;

    #[test]
    fn histogram_pb_round_trips_through_histogram() {
        let mut histogram = sig_fig_histogram::Histogram::new(2);
        histogram.observe_n(1.0, 3).unwrap();
        histogram.observe_n(12.0, 4).unwrap();

        let pb = HistogramPb::from(histogram.clone());
        let round_trip = pb.try_into_histogram().unwrap();

        assert_eq!(
            histogram.iter().collect::<Vec<_>>(),
            round_trip.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn histogram_pb_rejects_invalid_sig_figs() {
        let pb = HistogramPb::new(0, vec![]);

        assert_eq!(
            "sig figs out of bounds",
            pb.try_into_histogram().unwrap_err()
        );
    }

    #[test]
    fn sensor_reading_histogram_packs_and_unpacks() {
        let reading = SensorReading::Histogram(HistogramPb::new(2, vec![0, 1, 3]));
        let buf = stack_pack(&reading).to_vec();
        let mut unpacker = Unpacker::new(&buf);

        let unpacked: SensorReading = unpacker.unpack().unwrap();

        assert_eq!(reading, unpacked);
        assert_eq!(&[] as &[u8], unpacker.remain());
    }
}
