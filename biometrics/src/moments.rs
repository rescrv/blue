//! Statistical moments.

use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Moments are the statistical moments of mean (m1), standard deviation (m2), skewness (m3) and
/// kurtosis (m4).  When a distribution goes long tailed, skewness and kurtosis blow up, so the
/// hope is that this type will be good for monitoring general "no bad tail" insights.
///
/// The type itself is algebraic.  Take two readings separated by time and subtract them to get
/// perfectly recorded moments for the interval between the points.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Moments {
    /// The number of observations.
    pub n: u64,
    /// A value used to calculate mean.  It is mean.
    pub m1: f64,
    /// A value used to calculate variance.  It is not variance.
    pub m2: f64,
    /// A value used to calculate skewness.  It is not skewness.
    pub m3: f64,
    /// A value used to calculate kurtosis.  It is not kurtosis.
    pub m4: f64,
}

impl Moments {
    /// Create a new [Moments] with zero values.
    pub const fn new() -> Self {
        Moments {
            n: 0,
            m1: 0.0,
            m2: 0.0,
            m3: 0.0,
            m4: 0.0,
        }
    }

    /// Add `x` to the sequence of values the moments captures.
    pub fn push(&mut self, x: f64) {
        let n1: f64 = self.n as f64;
        self.n += 1;
        let n: f64 = self.n as f64;
        let delta: f64 = x - self.m1;
        let delta_n: f64 = delta / n;
        let delta_n2: f64 = delta_n * delta_n;
        let term1 = delta * delta_n * n1;
        self.m4 += term1 * delta_n2 * (n * n - 3. * n + 3.) + 6. * delta_n2 * self.m2
            - 4. * delta_n * self.m3;
        self.m3 += term1 * delta_n * (n - 2.) - 3. * delta_n * self.m2;
        self.m2 += term1;
        self.m1 += delta_n;
    }

    /// How many values does this statistical summary represent?
    pub fn n(&self) -> u64 {
        self.n
    }

    /// Average of values.
    pub fn mean(&self) -> f64 {
        self.m1
    }

    /// Square of standard deviation.
    pub fn variance(&self) -> f64 {
        self.m2 / (self.n as f64 - 1.)
    }

    /// How much the distance moves one way or another.
    pub fn skewness(&self) -> f64 {
        (self.n as f64).sqrt() * self.m3 / self.m2.powf(1.5)
    }

    /// Say something about the tail of the distribution.
    pub fn kurtosis(&self) -> f64 {
        (self.n as f64) * self.m4 / (self.m2 * self.m2)
    }

    /// Add two [Moments] together to get the equivalent of one that had the same set of values
    /// pushed.
    pub fn add(lhs: &Self, rhs: &Self) -> Self {
        if lhs.n == 0 {
            return *rhs;
        }
        if rhs.n == 0 {
            return *lhs;
        }
        let delta: f64 = rhs.m1 - lhs.m1;
        let delta2: f64 = delta * delta;
        let delta3: f64 = delta * delta2;
        let delta4: f64 = delta2 * delta2;
        let lhs_n: f64 = lhs.n as f64;
        let rhs_n: f64 = rhs.n as f64;
        let n: f64 = (lhs.n + rhs.n) as f64;
        let m1: f64 = (lhs_n * lhs.m1 + rhs_n * rhs.m1) / n;
        let m2: f64 = lhs.m2 + rhs.m2 + delta2 * lhs_n * rhs_n / n;
        let m3: f64 = lhs.m3
            + rhs.m3
            + delta3 * lhs_n * rhs_n * (lhs_n - rhs_n) / (n * n)
            + 3. * delta * (lhs_n * rhs.m2 - rhs_n * lhs.m2) / n;
        let m4: f64 = lhs.m4
            + rhs.m4
            + delta4 * lhs_n * rhs_n * (lhs_n * lhs_n - lhs_n * rhs_n + rhs_n * rhs_n)
                / (n * n * n)
            + 6. * delta2 * (lhs_n * lhs_n * rhs.m2 + rhs_n * rhs_n * lhs.m2) / (n * n)
            + 4. * delta * (lhs_n * rhs.m3 - rhs_n * lhs.m3) / n;
        Self {
            n: lhs.n + rhs.n,
            m1,
            m2,
            m3,
            m4,
        }
    }

    /// Compute `lhs - rhs`.
    ///
    /// This operation treats `rhs` as an earlier cumulative reading and `lhs` as a later
    /// cumulative reading.  The result is the moments for the observations that occurred between
    /// the two readings.
    ///
    /// # Panics
    ///
    /// Panics if `rhs` contains more observations than `lhs`.
    pub fn sub(lhs: &Self, rhs: &Self) -> Self {
        assert!(
            lhs.n >= rhs.n,
            "cannot subtract a moments reading with more observations"
        );
        if rhs.n == 0 {
            return *lhs;
        }
        if lhs.n == rhs.n {
            return Self::new();
        }
        let lhs_n: f64 = lhs.n as f64;
        let rhs_n: f64 = rhs.n as f64;
        let n: f64 = (lhs.n - rhs.n) as f64;
        let m1: f64 = (lhs_n * lhs.m1 - rhs_n * rhs.m1) / n;
        let delta: f64 = rhs.m1 - m1;
        let delta2: f64 = delta * delta;
        let delta3: f64 = delta * delta2;
        let delta4: f64 = delta2 * delta2;
        let m2: f64 = lhs.m2 - rhs.m2 - delta2 * rhs_n * n / lhs_n;
        let m3: f64 = lhs.m3
            - rhs.m3
            - delta3 * rhs_n * n * (n - rhs_n) / lhs_n.powf(2.)
            - 3. * delta * (n * rhs.m2 - rhs_n * m2) / lhs_n;
        let m4: f64 = lhs.m4
            - rhs.m4
            - delta4 * n * rhs_n * (n * n - n * rhs_n + rhs_n * rhs_n) / (lhs_n * lhs_n * lhs_n)
            - 6. * delta2 * (n * n * rhs.m2 + rhs_n * rhs_n * m2) / (lhs_n * lhs_n)
            - 4. * delta * (n * rhs.m3 - rhs_n * m3) / lhs_n;
        Self {
            n: lhs.n - rhs.n,
            m1,
            m2,
            m3,
            m4,
        }
    }
}

impl Add<Moments> for Moments {
    type Output = Self;

    fn add(self, other: Moments) -> Self {
        Self::add(&self, &other)
    }
}

impl AddAssign<Moments> for Moments {
    fn add_assign(&mut self, other: Moments) {
        *self = Self::add(self, &other);
    }
}

impl Sub<Moments> for Moments {
    type Output = Self;

    fn sub(self, other: Moments) -> Self {
        Self::sub(&self, &other)
    }
}

impl SubAssign<Moments> for Moments {
    fn sub_assign(&mut self, other: Moments) {
        *self = Self::sub(self, &other);
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    fn moments_from(values: &[f64]) -> Moments {
        let mut moments = Moments::new();
        for value in values {
            moments.push(*value);
        }
        moments
    }

    fn assert_close(expected: f64, actual: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan(), "expected NaN, got {actual}");
        } else {
            assert!(
                (expected - actual).abs() <= EPSILON,
                "expected {expected}, got {actual}"
            );
        }
    }

    fn assert_moments_approx(expected: Moments, actual: Moments) {
        assert_eq!(expected.n, actual.n);
        assert_close(expected.m1, actual.m1);
        assert_close(expected.m2, actual.m2);
        assert_close(expected.m3, actual.m3);
        assert_close(expected.m4, actual.m4);
    }

    #[test]
    fn moments_may_be_static() {
        static MOMENTS: Moments = Moments::new();
        assert_eq!(Moments::new(), MOMENTS);
    }

    #[test]
    fn empty_moments_are_zero() {
        assert_eq!(
            Moments {
                n: 0,
                m1: 0.0,
                m2: 0.0,
                m3: 0.0,
                m4: 0.0,
            },
            Moments::new()
        );
    }

    #[test]
    fn one_observation_has_undefined_sample_statistics() {
        let moments = moments_from(&[42.0]);
        assert_eq!(
            Moments {
                n: 1,
                m1: 42.0,
                m2: 0.0,
                m3: 0.0,
                m4: 0.0,
            },
            moments
        );
        assert!(moments.variance().is_nan());
        assert!(moments.skewness().is_nan());
        assert!(moments.kurtosis().is_nan());
    }

    #[test]
    fn add_empty_is_identity() {
        let populated = moments_from(&[1.0, 3.0, 5.0]);

        assert_eq!(populated, Moments::add(&Moments::new(), &populated));
        assert_eq!(populated, Moments::add(&populated, &Moments::new()));
        assert_eq!(
            Moments::new(),
            Moments::add(&Moments::new(), &Moments::new())
        );
    }

    #[test]
    fn add_matches_incremental_push() {
        let lhs = moments_from(&[1.0, 2.0, 3.0]);
        let rhs = moments_from(&[10.0, 20.0]);
        let expected = moments_from(&[1.0, 2.0, 3.0, 10.0, 20.0]);

        assert_moments_approx(expected, Moments::add(&lhs, &rhs));
    }

    #[test]
    fn sub_extracts_interval_between_readings() {
        let earlier = moments_from(&[1.0, 2.0, 3.0]);
        let interval = moments_from(&[5.0, 8.0, 13.0]);
        let later = Moments::add(&earlier, &interval);

        assert_moments_approx(interval, Moments::sub(&later, &earlier));
    }

    #[test]
    fn sub_with_no_new_observations_is_empty() {
        let reading = moments_from(&[1.0, 2.0, 3.0]);

        assert_eq!(Moments::new(), Moments::sub(&reading, &reading));
    }

    #[test]
    #[should_panic(expected = "cannot subtract a moments reading with more observations")]
    fn sub_panics_when_rhs_has_more_observations() {
        let earlier = moments_from(&[1.0, 2.0, 3.0]);
        let later = moments_from(&[1.0]);

        let _ = Moments::sub(&later, &earlier);
    }
}
