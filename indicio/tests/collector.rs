use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use indicio::{ALWAYS, Collector, Emitter, Value, clue};

#[derive(Default)]
struct RecordingEmitter {
    emitted: Mutex<Vec<(String, u32, u64, Value)>>,
}

impl Emitter for RecordingEmitter {
    fn emit(&self, file: &str, line: u32, level: u64, value: Value) {
        self.emitted
            .lock()
            .unwrap()
            .push((file.to_string(), line, level, value));
    }
}

fn expensive(flag: &AtomicBool) -> &'static str {
    flag.store(true, Ordering::SeqCst);
    "value"
}

#[test]
fn clue_accepts_collector_expressions_and_remains_lazy() {
    let collector = Collector::new();
    let evaluated = AtomicBool::new(false);

    clue!(&collector, ALWAYS, {
        value: expensive(&evaluated),
    });
    assert!(!evaluated.load(Ordering::SeqCst));

    let emitter = Arc::new(RecordingEmitter::default());
    collector.register_arc(emitter.clone());
    collector.set_verbosity(ALWAYS);
    clue!(&collector, ALWAYS, {
        value: expensive(&evaluated),
    });

    assert!(evaluated.load(Ordering::SeqCst));
    let mut emitted = emitter.emitted.lock().unwrap().clone();
    for clue in emitted.iter_mut() {
        clue.0 = "loc".to_string();
        clue.1 = 0;
    }
    assert_eq!(
        vec![(
            "loc".to_string(),
            0,
            ALWAYS,
            indicio::value!({ value: "value" })
        )],
        emitted
    );
}

struct DeregisteringEmitter {
    collector: Arc<Collector>,
    emitted: Arc<AtomicBool>,
}

impl Emitter for DeregisteringEmitter {
    fn emit(&self, _file: &str, _line: u32, _level: u64, _value: Value) {
        self.emitted.store(true, Ordering::SeqCst);
        self.collector.deregister();
    }
}

#[test]
fn emitter_can_reenter_collector_without_deadlock() {
    let collector = Arc::new(Collector::new());
    let emitted = Arc::new(AtomicBool::new(false));
    collector.register(DeregisteringEmitter {
        collector: Arc::clone(&collector),
        emitted: Arc::clone(&emitted),
    });
    collector.set_verbosity(ALWAYS);

    collector.emit("file", 1, ALWAYS, Value::Bool(true));

    assert!(emitted.load(Ordering::SeqCst));
    assert!(!collector.is_logging());
}
