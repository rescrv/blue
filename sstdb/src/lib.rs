//! sstdb — a single-SST, linearizable, capacity-bounded key-value database that lives entirely on
//! object storage.
//!
//! The entire live dataset is one SST object (`db/sst/CURRENT`).  Writes are single-stepped: a
//! writer claims an offset by writing an immutable log fragment with `If-None-Match`, then rolls
//! the whole SST forward by applying that fragment and installing the result with `If-Match`.
//! Reads fetch the whole SST.  **Conditional PUT is the only synchronization mechanism in the
//! system** — there is no external coordination service.
//!
//! The crate is organized into the following pieces:
//!
//! * [`ConditionalStore`] — the thin object-store abstraction exposing exactly the three
//!   conditional operations the design needs ([`store`]).
//! * [`Snapshot`] — the materialized database, (de)serialized to/from a real SST via the `sst`
//!   crate, with in-keyspace meta keys ([`snapshot`]).
//! * [`Reader`] — the 100 ms read-coalescing window ([`reader`]).
//! * [`Database`] — the read path, the write path (fragment claim → rollup → install), recovery
//!   (writer-driven roll-forward), and ETag-as-read-version transactions ([`db`]).

mod db;
mod fragment;
mod reader;
mod snapshot;
mod store;

pub use db::{Database, DatabaseOptions, LogPosition, Transaction};
pub use reader::Reader;
pub use snapshot::{META_PREFIX, Snapshot};
pub use sst::log::WriteBatch;
pub use store::{ConditionalStore, Object, ObjectStoreConditional, WriteOutcome};

/// The reserved meta-key prefix length: any key beginning with this many `0xff` bytes is a meta
/// key and is not part of the user keyspace.
pub const META_PREFIX_LEN: usize = 5;

pub use handled::SError;

/// The phase tag stamped on every [`SError`] this crate produces.
const ERROR_PHASE: &str = "sstdb";

/// The crate result type.  Every fallible operation reports failures as a [`handled::SError`]; the
/// `code` field discriminates the cases the old hand-rolled enum used to express.
pub type Result<T> = std::result::Result<T, SError>;

/// An object-store operation failed for a non-precondition reason (code `store-error`).
pub(crate) fn store_error(e: object_store::Error) -> SError {
    SError::new(ERROR_PHASE)
        .with_code("store-error")
        .with_message("object store operation failed")
        .with_debug_field("cause", e)
}

/// The proposed write would push the database over its configured capacity (code
/// `capacity-exceeded`).  `size` is the size the new SST would have had and `cap` the configured
/// limit, both in bytes.
pub(crate) fn capacity_exceeded(size: usize, cap: usize) -> SError {
    SError::new(ERROR_PHASE)
        .with_code("capacity-exceeded")
        .with_message("write would exceed configured capacity")
        .with_atom_field("size", size)
        .with_atom_field("cap", cap)
}

/// A client tried to write to or read across the reserved meta-key range (code `reserved-key`).
pub(crate) fn reserved_key() -> SError {
    SError::new(ERROR_PHASE)
        .with_code("reserved-key")
        .with_message("key is in the reserved meta-key range")
}

/// A client-supplied [`WriteBatch`] used a timestamp that was not strictly greater than the
/// database's high-water mark (code `invalid-timestamp`).  sstdb requires every write's timestamp
/// to exceed the highest timestamp it has incorporated, so the timestamps form a strictly
/// increasing total order supplied by the client and enforced by the database.  `timestamp` is the
/// offending entry's timestamp; `high_water_mark` is the value it had to beat.
pub(crate) fn invalid_timestamp(timestamp: u64, high_water_mark: u64) -> SError {
    SError::new(ERROR_PHASE)
        .with_code("invalid-timestamp")
        .with_message("sstdb write batches must use strictly-increasing timestamps")
        .with_atom_field("timestamp", timestamp)
        .with_atom_field("high-water-mark", high_water_mark)
}

/// A loaded SST's recomputed setsum did not match its stamped setsum: corruption (code
/// `setsum-mismatch`).  Per the design, this halts the writer for human attention.  `stamped` is
/// the setsum stamped in the SST's meta keys; `computed` is the setsum recomputed over the loaded
/// user keyspace.
pub(crate) fn setsum_mismatch(stamped: &str, computed: &str) -> SError {
    SError::new(ERROR_PHASE)
        .with_code("setsum-mismatch")
        .with_message("recomputed setsum disagrees with the stamped setsum")
        .with_string_field("stamped", stamped)
        .with_string_field("computed", computed)
}

/// An object or invariant that should have held did not (code `corruption`).
pub(crate) fn corruption(message: impl AsRef<str>) -> SError {
    SError::new(ERROR_PHASE)
        .with_code("corruption")
        .with_message(message.as_ref())
}

/// An optimistic transaction lost the first-committer-wins race: the SST moved underneath the
/// transaction's read-version (code `conflict`).  The caller should re-read, reapply its
/// read-modify-write logic, and retry (§7).
pub(crate) fn conflict() -> SError {
    SError::new(ERROR_PHASE)
        .with_code("conflict")
        .with_message("transaction conflict: the SST moved; reapply and retry")
}
