//! Manage background-threads.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

///////////////////////////////////////// BackgroundThread /////////////////////////////////////////

/// A background thread.  Joins on drop, so long as there's no panic in-progress.
pub struct BackgroundThread {
    done: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl BackgroundThread {
    /// Spawn a new background thread.
    // TODO(rescrv): Make this pass in something to call rather than an Arc<AtomicBool>.
    pub fn spawn<F: FnOnce(Arc<AtomicBool>) + Send + 'static>(f: F) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let done_p = Arc::clone(&done);
        let thread = Some(std::thread::spawn(move || f(done_p)));
        Self { done, thread }
    }

    /// Join the background thread, consuming it.
    pub fn join(self) {
        // Drop will join.
    }
}

impl Drop for BackgroundThread {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Relaxed);
        if !std::thread::panicking() {
            let _ = self.thread.take().unwrap().join();
        }
    }
}
