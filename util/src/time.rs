/// Deal with the here and now.
pub mod now {
    use std::time::{UNIX_EPOCH, SystemTime};

    /// Return the current number of milliseconds since the UNIX epoch.
    pub fn millis() -> u64 {
        (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX epoch")
            .as_secs_f64() * 1_000.0) as u64
    }

    /// Return the current number of microseconds since the UNIX epoch.
    pub fn micros() -> u64 {
        (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX epoch")
            .as_secs_f64() * 1_000_000.0) as u64
    }
}
