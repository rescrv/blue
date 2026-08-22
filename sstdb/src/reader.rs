//! The read path with a 100 ms read-coalescing window (§5).
//!
//! Because reads dominate 10:1 and a read is a full-SST fetch, concurrent reads over a short window
//! are satisfied by a single object-storage fetch and share the result.  This is the time-symmetric
//! mirror of wal3's write batching: there it accumulates *writes* to amortize a PUT; here it
//! accumulates *reads* to amortize a GET.
//!
//! There is exactly one rendezvous point — the open batch — so a single `Mutex` is the honest
//! primitive at cardinality one.  **The lock is never held across an await**: the batch is sealed
//! and the lock released *before* any object-store round trip, so a slow fetch never blocks new
//! readers from forming the next batch.
//!
//! **Every window talks to object storage.**  The HEAD optimization (§5) collapses a full GET into
//! a HEAD when the ETag is unchanged since the previous window, reusing the bytes already held — but
//! it is still a fetch-time check every window, never a window trusting an earlier window's `Arc`
//! without asking object storage.  Coalescing changes only cost, never what a reader sees.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::snapshot::Snapshot;
use crate::store::ConditionalStore;
use crate::{Result, SError, corruption};

/// The default read-batching window.
pub const DEFAULT_WINDOW: Duration = Duration::from_millis(100);

type Waiter = oneshot::Sender<std::result::Result<Arc<Snapshot>, Arc<SError>>>;

/// The open batch: the set of readers awaiting the current window plus the window's arm-state.
struct Batch {
    waiters: Vec<Waiter>,
    armed: bool,
}

impl Batch {
    fn empty() -> Self {
        Batch {
            waiters: Vec::new(),
            armed: false,
        }
    }
}

struct ReaderInner {
    store: Arc<dyn ConditionalStore>,
    slot: String,
    window: Duration,
    verify: bool,
    // Held only to mutate the waiter set; never across an await.
    batch: std::sync::Mutex<Batch>,
    // The previous window's fetch, for the HEAD optimization.  tokio mutex because it is touched
    // across awaits in the driver.
    cached: tokio::sync::Mutex<Option<(String, Arc<Snapshot>)>>,
}

/// The read-coalescing reader.  Cheap to clone; clones share one window.
#[derive(Clone)]
pub struct Reader {
    inner: Arc<ReaderInner>,
}

impl Reader {
    /// Create a reader over `store`, resolving the SST slot at `slot`, batching over `window`.  If
    /// `verify` is set, every fetched SST has its setsum verified (the opt-in paranoid read of §8).
    pub fn new(
        store: Arc<dyn ConditionalStore>,
        slot: impl Into<String>,
        window: Duration,
        verify: bool,
    ) -> Self {
        Reader {
            inner: Arc::new(ReaderInner {
                store,
                slot: slot.into(),
                window,
                verify,
                batch: std::sync::Mutex::new(Batch::empty()),
                cached: tokio::sync::Mutex::new(None),
            }),
        }
    }

    /// Join the open batch and await the window's shared SST.  Every in-flight request is satisfied
    /// by its own window's fetch against object storage.
    pub async fn current(&self) -> Result<Arc<Snapshot>> {
        let (tx, rx) = oneshot::channel();
        let arm = {
            let mut b = self.inner.batch.lock().unwrap();
            b.waiters.push(tx);
            let first = !b.armed;
            b.armed = true;
            first
        }; // <- lock released here
        if arm {
            let inner = Arc::clone(&self.inner);
            let window = inner.window;
            tokio::spawn(async move {
                tokio::time::sleep(window).await;
                Reader::drive(inner).await;
            });
        }
        match rx.await {
            Ok(Ok(snap)) => Ok(snap),
            Ok(Err(shared)) => Err((*shared).clone()),
            Err(_canceled) => Err(corruption("read window driver dropped before resolving")),
        }
    }

    /// Seal the batch, resolve exactly one SST, and distribute `Arc<Snapshot>` clones to every
    /// sealed waiter.  Readers arriving after the seal form the next window.
    async fn drive(inner: Arc<ReaderInner>) {
        let sealed = {
            let mut b = inner.batch.lock().unwrap();
            std::mem::replace(&mut *b, Batch::empty())
        }; // <- lock released before any I/O
        let resolved = Reader::resolve_current(&inner).await;
        match resolved {
            Ok(snap) => {
                for tx in sealed.waiters {
                    let _ = tx.send(Ok(Arc::clone(&snap)));
                }
            }
            Err(e) => {
                let shared = Arc::new(e);
                for tx in sealed.waiters {
                    let _ = tx.send(Err(Arc::clone(&shared)));
                }
            }
        }
    }

    /// Resolve the current SST.  Uses the HEAD optimization: if the slot's ETag is unchanged since
    /// the previous window, reuse the cached `Arc<Snapshot>` (HEAD only, no byte transfer);
    /// otherwise GET and parse.  A missing slot resolves to the empty genesis snapshot.
    async fn resolve_current(inner: &ReaderInner) -> Result<Arc<Snapshot>> {
        let prev = inner.cached.lock().await.clone();
        if let Some((prev_etag, prev_snap)) = prev {
            match inner.store.head(&inner.slot).await? {
                Some(etag) if etag == prev_etag => {
                    // Unchanged since our previous fetch: reuse, no byte transfer.
                    return Ok(prev_snap);
                }
                _ => {} // changed or vanished: fall through to a full GET.
            }
        }
        match inner.store.get(&inner.slot).await? {
            Some(obj) => {
                let snap = Snapshot::from_sst_bytes(&obj.bytes, inner.verify)?;
                let snap = Arc::new(snap);
                *inner.cached.lock().await = Some((obj.etag, Arc::clone(&snap)));
                Ok(snap)
            }
            None => {
                // No slot yet: serve the empty genesis.  Do not cache (no ETag to key on).
                Ok(Arc::new(Snapshot::empty()))
            }
        }
    }
}
