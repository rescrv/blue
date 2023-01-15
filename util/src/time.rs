pub mod now {
    use std::time::{UNIX_EPOCH, SystemTime};

    pub fn millis() -> u64 {
        (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX epoch")
            .as_secs_f64() * 1_000.0) as u64
    }

    pub fn micros() -> u64 {
        (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX epoch")
            .as_secs_f64() * 1_000_000.0) as u64
    }
}
