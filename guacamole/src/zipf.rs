use super::{FromGuacamole, Guacamole};

/////////////////////////////////////////////// Zipf ///////////////////////////////////////////////

/// Zipf generator over [0, n).  From:
///
/// "Quickly Generating Billion-Record Synthetic Databases."
/// Gray et.al., SIGMOD 1994
///
/// This should be used *only* where it's OK to not be a perfect Zipf distribution.  For larger
/// distributions, low-rank items are missing and the curve is bent.  It's an approximation meant
/// to skew workloads for a key-value-store generator.
#[derive(Clone, Debug)]
pub struct Zipf {
    n: u64,
    alpha: f64,
    theta: f64,
    zetan: f64,
    zeta2: f64,
    eta: f64,
}

impl Zipf {
    /// Create a new Zipf distribution for `n` objects with `alpha > 1`.
    #[deprecated(since = "0.12.0", note = "Use `from_param` instead")]
    pub fn from_alpha(n: u64, alpha: f64) -> Self {
        let mut zipf = Zipf {
            n,
            alpha,
            theta: 1.0 - 1.0 / alpha,
            zetan: 0.0,
            zeta2: 0.0,
            eta: 0.0,
        };
        zipf.init();
        zipf
    }

    /// Create a new Zipf distribution for `n` objects with `theta` over `(0, 1)`.
    #[deprecated(since = "0.12.0", note = "Use `from_param` instead")]
    pub fn from_theta(n: u64, theta: f64) -> Self {
        let mut zipf = Zipf {
            n,
            theta,
            alpha: 1.0 / (1.0 - theta),
            zetan: 0.0,
            zeta2: 0.0,
            eta: 0.0,
        };
        zipf.init();
        zipf
    }

    /// Create a new Zipf distribution for `n` objects with `skew` parameter over `(0, 1)`.
    /// The `skew` parameter functions identically to the deprecated `theta` parameter.
    pub fn from_param(n: u64, skew: f64) -> Self {
        let mut zipf = Zipf {
            n,
            theta: skew,
            alpha: 1.0 / (1.0 - skew),
            zetan: 0.0,
            zeta2: 0.0,
            eta: 0.0,
        };
        zipf.init();
        zipf
    }

    /// Use `guac` to generate some randomness and then adjust so that the returned u64 obeys a
    /// Zipf distribution on [1, n], where 1 is the most common element, 2 the next most common and
    /// so on.  It's not perfect, so expect to see cases where the distribution doesn't hold.
    pub fn next(&self, guac: &mut Guacamole) -> u64 {
        let u: f64 = f64::from_guacamole(&mut (), guac);
        let uz: f64 = u * self.zetan;
        if uz < 1.0 {
            return 1;
        }
        if uz < 1.0 + 0.5_f64.powf(self.theta) {
            return 2;
        }
        let scale: f64 = (self.eta * u - self.eta + 1.0).powf(self.alpha);
        let result = 1 + (self.n as f64 * scale) as u64;
        result.min(self.n)
    }

    fn zeta(n: u64, theta: f64) -> f64 {
        let mut sum: f64 = 0.0;

        for i in 0..n {
            let x: f64 = i as f64 + 1.0;
            sum += 1.0 / x.powf(theta);
        }

        sum
    }

    fn zeta_approx(n: u64, theta: f64) -> f64 {
        if n <= 1000 {
            return Self::zeta(n, theta);
        }

        let n_f = n as f64;
        let integral = if theta == 1.0 {
            n_f.ln()
        } else {
            (n_f.powf(1.0 - theta) - 1.0) / (1.0 - theta)
        };

        integral + 1.0 + 0.5 * n_f.powf(-theta) - theta / 12.0 * n_f.powf(-theta - 1.0)
    }

    fn init(&mut self) {
        self.zetan = Self::zeta_approx(self.n, self.theta);
        self.zeta2 = Self::zeta(2, self.theta);
        self.eta =
            (1.0 - (2.0 / self.n as f64).powf(1.0 - self.theta)) / (1.0 - self.zeta2 / self.zetan);
    }
}
