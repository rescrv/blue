//! Statistical moments.

use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Moments are the statistical moments of mean (m1), standard deviation (m2), skewness (m3) and
/// kurtosis (m4).  When a distribution goes long tailed, skewness and kurtosis blow up, so the
/// hope is that this type will be good for monitoring general "no bad tail" insights.
///
/// The type itelf is algebraic.  Take two readings separated by time and subtract them to get
/// perfectly recorded moments for the interval between the points.
#[derive(Clone, Copy, Default, Debug)]
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

    /// Compute lhs - rhs.
    pub fn sub(lhs: &Self, rhs: &Self) -> Self {
        let lhs_n: f64 = lhs.n as f64;
        let rhs_n: f64 = rhs.n as f64;
        let n: f64 = (rhs.n - lhs.n) as f64;
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

    #[test]
    fn moments_may_be_static() {
        static MOMENTS: Moments = Moments::new();
        println!(
            "MOMENTS = {} {} {} {}",
            MOMENTS.n(),
            MOMENTS.mean(),
            MOMENTS.skewness(),
            MOMENTS.kurtosis()
        );
    }
}
