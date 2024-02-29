//! Biometrics emitters for workloads.

use std::fs::File;
use std::io::Write;
use std::sync::Mutex;

use biometrics::{Counter, Emitter, Gauge, Moments, Sensor};

///////////////////////////////////////// PlainTextEmitter /////////////////////////////////////////

/// An emitter that puts readings one-per-line.
///
/// This differs from the biometrics emitter because it will start from 0 every time instead of
/// emitting the current time.
pub struct PlainTextEmitter {
    output: File,
    offset: Mutex<Option<u64>>,
}

impl PlainTextEmitter {
    /// Create a new emitter with the output file.
    pub fn new(output: File) -> Self {
        let offset = Mutex::new(None);
        Self { output, offset }
    }

    fn offset(&self, now: u64) -> u64 {
        let mut offset = self.offset.lock().unwrap();
        if offset.is_none() {
            *offset = Some(now);
        }
        // SAFETY(rescrv): Offset is guaranteed some at this point, so unwrap is safe.
        (now - offset.unwrap()) / 1_000
    }
}

impl Emitter for PlainTextEmitter {
    type Error = std::io::Error;

    fn emit_counter(&mut self, counter: &Counter, now: u64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!(
            "{} {} {}\n",
            counter.label(),
            self.offset(now),
            counter.read()
        ))
    }

    fn emit_gauge(&mut self, gauge: &Gauge, now: u64) -> Result<(), std::io::Error> {
        self.output.write_fmt(format_args!(
            "{} {} {}\n",
            gauge.label(),
            self.offset(now),
            gauge.read()
        ))
    }

    fn emit_moments(&mut self, moments: &Moments, now: u64) -> Result<(), std::io::Error> {
        let label = moments.label();
        let moments = moments.read();
        self.output.write_fmt(format_args!(
            "{} {} {} {} {} {} {}\n",
            label,
            self.offset(now),
            moments.n,
            moments.m1,
            moments.m2,
            moments.m3,
            moments.m4,
        ))
    }
}
