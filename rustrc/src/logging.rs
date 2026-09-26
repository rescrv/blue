//! An indicio emitter that never blocks the thread that logs.
//!
//! rustrc logs while holding its state lock.  indicio's StdioEmitter writes to stderr inline (and
//! `eprintln!` panics if the write fails), so a stalled log reader, such as a full pipe to a
//! container runtime, would wedge the supervisor, and a closed stderr would crash it.  This emitter
//! formats the line on the calling thread, hands it to a bounded queue, and drops it if the queue is
//! full.  A dedicated thread does the writing.  Dropped lines are counted in `rustrc.log.dropped` and
//! announced in the log once there is room again.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, SystemTime};

use indicio::{Emitter, Value};

static LOG_DROPPED: biometrics::Counter = biometrics::Counter::new("rustrc.log.dropped");

pub(crate) fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_counter(&LOG_DROPPED);
}

/// An [Emitter] that queues formatted lines for a writer thread and never blocks.
#[derive(Debug)]
pub struct QueuedEmitter {
    tx: SyncSender<String>,
    dropped: AtomicU64,
}

/// Returned by [QueuedEmitter::new]; waits for the writer thread to drain.
#[derive(Debug)]
pub struct QueuedWriter {
    done: Receiver<()>,
}

impl QueuedEmitter {
    /// Start a writer thread that writes to `out`, buffering at most `capacity` lines.
    pub fn new<W: Write + Send + 'static>(
        mut out: W,
        capacity: usize,
    ) -> std::io::Result<(Self, QueuedWriter)> {
        let (tx, rx) = sync_channel::<String>(capacity);
        let (done_tx, done) = sync_channel(1);
        std::thread::Builder::new()
            .name("rustrc-log".to_string())
            .spawn(move || {
                for line in rx {
                    // Nowhere to report a failed log write; keep draining so senders never block.
                    let _ = out.write_all(line.as_bytes());
                }
                let _ = out.flush();
                let _ = done_tx.send(());
            })?;
        let dropped = AtomicU64::new(0);
        Ok((Self { tx, dropped }, QueuedWriter { done }))
    }

    fn offer(&self, line: String) -> bool {
        match self.tx.try_send(line) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                LOG_DROPPED.click();
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }
}

impl Emitter for QueuedEmitter {
    fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
        let level = match level {
            0 => "A",
            1..=3 => "E",
            4..=6 => "W",
            7..=9 => "I",
            10..=12 => "D",
            _ => "T",
        };
        let dropped = self.dropped.swap(0, Ordering::Relaxed);
        if dropped > 0 {
            let notice = format!("W {:17.6} rustrc: dropped {dropped} log lines\n", now());
            if !self.offer(notice) {
                self.dropped.fetch_add(dropped, Ordering::Relaxed);
            }
        }
        self.offer(format!("{level} {:17.6} {file}:{line} {value}\n", now()));
    }
}

impl QueuedWriter {
    /// Wait up to `timeout` for the writer to drain and exit.  The writer exits once every
    /// [QueuedEmitter] is dropped (for the global collector, after `COLLECTOR.deregister()`).
    /// False if it did not finish in time, e.g. because the output is blocked.
    pub fn finish(self, timeout: Duration) -> bool {
        self.done.recv_timeout(timeout).is_ok()
    }
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|x| x.as_micros() as f64 / 1_000_000.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use biometrics::Sensor;

    use super::*;

    /// A writer that blocks until the gate opens.
    struct Gated {
        gate: Receiver<()>,
        open: bool,
        out: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for Gated {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if !self.open {
                let _ = self.gate.recv();
                self.open = true;
            }
            self.out.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_stalled_writer_never_blocks_the_logger() {
        let (open, gate) = sync_channel(1);
        let out = Arc::new(Mutex::new(vec![]));
        let writer = Gated {
            gate,
            open: false,
            out: Arc::clone(&out),
        };
        let (emitter, drained) = QueuedEmitter::new(writer, 8).unwrap();
        let before = LOG_DROPPED.read();
        let start = Instant::now();
        for i in 0..10_000u64 {
            emitter.emit("f.rs", 1, 9, Value::from(i));
        }
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(LOG_DROPPED.read() > before);
        open.send(()).unwrap();
        // Let the queue drain so the next line and its drop notice fit.
        std::thread::sleep(Duration::from_millis(100));
        emitter.emit("f.rs", 2, 9, Value::from("after"));
        drop(emitter);
        assert!(drained.finish(Duration::from_secs(5)));
        let out = String::from_utf8(out.lock().unwrap().clone()).unwrap();
        assert!(out.contains("rustrc: dropped "), "{out}");
        assert!(out.lines().last().unwrap().contains("f.rs:2"), "{out}");
    }
}
