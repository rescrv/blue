use std::time::Instant;

///////////////////////////////////////////// Stopwatch ////////////////////////////////////////////

pub struct Stopwatch {
    start: Instant,
}

impl Stopwatch {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn since(&self) -> f64 {
        self.start.elapsed().as_millis() as f64 / 1_000.
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
        let stopwatch = Stopwatch::new();
        sleep(Duration::from_millis(1));
        assert!(stopwatch.since() > 0.0);
        sleep(Duration::from_millis(100));
        assert!(stopwatch.since() > 0.1);
        sleep(Duration::from_millis(1000));
        assert!(stopwatch.since() > 1.1);
    }
}
