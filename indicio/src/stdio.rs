use std::time::SystemTime;

use super::*;

/// An Emitter that writes clues to stderr.  When the file reaches its size
/// threshold, it rolls over to the next file.
pub struct StdioEmitter;

impl Emitter for StdioEmitter {
    fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|x| x.as_micros() as f64 / 1_000.0)
            .unwrap_or(0.0);
        let level = match level {
            0 => "A",
            1..=3 => "E",
            4..=6 => "W",
            7..=9 => "I",
            10..=12 => "D",
            13..=15 => "T",
            _ => {
                return;
            }
        };
        eprintln!("{level} {timestamp:10.3} {file}:{line} {value}");
    }

    fn flush(&self) {}
}
