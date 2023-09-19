//! A Stopwatch.

use std::time::Instant;

///////////////////////////////////////////// Stopwatch ////////////////////////////////////////////

/// A stopwatch measuring from one instant to another.  A loose wrapper around std::time::Instant.
pub struct Stopwatch {
    start: Instant,
}

impl Stopwatch {
    /// Measure the time that the stopwatch has been running, as a value suitable for putting into
    /// e.g. a gauge.
    pub fn since(&self) -> f64 {
        self.start.elapsed().as_micros() as f64 / 1_000_000.0
    }
}

impl Default for Stopwatch {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::thread::sleep;
    use std::time::Duration;

    use super::*;

    #[test]
    fn it_works() {
        let stopwatch = Stopwatch::default();
        sleep(Duration::from_millis(1));
        assert!(stopwatch.since() > 0.0);
        sleep(Duration::from_millis(100));
        assert!(stopwatch.since() > 0.1);
        sleep(Duration::from_millis(1000));
        assert!(stopwatch.since() > 1.1);
    }
}
