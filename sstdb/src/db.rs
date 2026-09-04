//! The [`Database`]: read path, write path, recovery, and transactions.
//!
//! This ties together the conditional store (§3), the materialized snapshot (§4.1, §8), and the
//! read-coalescing reader (§5) into the single-stepped write loop (§6), the writer-driven
//! roll-forward recovery (§6.1), and the ETag-as-read-version transactions (§7).
//!
//! Object layout (§4):
//!
//! ```text
//! <prefix>/fragment/<016x offset>   # immutable, If-None-Match claim
//! <prefix>/sst/CURRENT              # the live SST; CAS'd via If-Match
//! ```

use std::ops::Bound;
use std::sync::Arc;
use std::time::Duration;

use sst::Builder;
use sst::log::WriteBatch;

use crate::fragment::{check_batch, decode, encode};
use crate::reader::{DEFAULT_WINDOW, Reader};
use crate::snapshot::Snapshot;
use crate::store::{ConditionalStore, WriteOutcome};
use crate::{Result, capacity_exceeded, conflict, setsum_mismatch};

/// The default capacity cap: 32 MiB, the expected operating regime given "most reads under 32 MB"
/// (§9).
pub const DEFAULT_CAP: usize = 32 * 1024 * 1024;

/// The position a write was assigned in the total order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogPosition {
    /// The fragment offset this write claimed (and at which it linearized).
    pub offset: u64,
}

/// Configuration for a [`Database`].
#[derive(Clone, Debug)]
pub struct DatabaseOptions {
    /// The object-key prefix under which fragments and the SST slot live.
    pub prefix: String,
    /// The capacity cap, in bytes (§9).  Writes whose resulting SST would exceed this are rejected.
    pub cap: usize,
    /// The read-coalescing window (§5).
    pub window: Duration,
    /// Whether coalesced reads verify the SST setsum on fetch (the opt-in paranoid read of §8).
    pub verify_reads: bool,
}

impl Default for DatabaseOptions {
    fn default() -> Self {
        DatabaseOptions {
            prefix: "db".to_string(),
            cap: DEFAULT_CAP,
            window: DEFAULT_WINDOW,
            verify_reads: false,
        }
    }
}

/// A snapshot of the SST slot together with its ETag (the transaction read-version), or `None` for
/// the ETag if the slot does not yet exist (genesis).
struct CurrentState {
    snapshot: Snapshot,
    etag: Option<String>,
}

/// The single-SST database.
#[derive(Clone)]
pub struct Database {
    store: Arc<dyn ConditionalStore>,
    reader: Reader,
    options: DatabaseOptions,
    slot_path: String,
    fragment_prefix: String,
}

impl Database {
    /// Open a database over `store` with `options`.  Nothing is read or written until the first
    /// operation; an absent slot is treated as the empty genesis.
    pub fn open(store: Arc<dyn ConditionalStore>, options: DatabaseOptions) -> Self {
        let slot_path = format!("{}/sst/CURRENT", options.prefix);
        let fragment_prefix = format!("{}/fragment", options.prefix);
        let reader = Reader::new(
            Arc::clone(&store),
            slot_path.clone(),
            options.window,
            options.verify_reads,
        );
        Database {
            store,
            reader,
            options,
            slot_path,
            fragment_prefix,
        }
    }

    /// The object path for a fragment at `offset`.  The path is fully specified so future layout
    /// changes do not invalidate existing data (§4).
    fn fragment_path(&self, offset: u64) -> String {
        format!("{}/{:016x}", self.fragment_prefix, offset)
    }

    // ----------------------------------------------------------------------------- read path

    /// Fetch the current snapshot through the 100 ms read-coalescing window (§5).
    pub async fn snapshot(&self) -> Result<Arc<Snapshot>> {
        self.reader.current().await
    }

    /// Point lookup.  Returns the value for `key`, or `None` if absent.  Meta keys are never
    /// visible.
    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let snap = self.snapshot().await?;
        Ok(snap.get(key).map(|v| v.to_vec()))
    }

    /// Range scan, returning owned key/value pairs in sorted order over `[start, end)`-style bounds.
    pub async fn scan<S, E>(
        &self,
        start: Bound<S>,
        end: Bound<E>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>>
    where
        S: AsRef<[u8]>,
        E: AsRef<[u8]>,
    {
        let snap = self.snapshot().await?;
        Ok(snap.scan(start, end))
    }

    // ----------------------------------------------------------------------------- write path

    /// Load the current SST slot directly (bypassing the read window) with writer-side setsum
    /// verification (§8).  A missing slot resolves to the empty genesis with no ETag.
    async fn load_current(&self) -> Result<CurrentState> {
        match self.store.get(&self.slot_path).await? {
            Some(obj) => {
                let snapshot = Snapshot::from_sst_bytes(&obj.bytes, true)?;
                Ok(CurrentState {
                    snapshot,
                    etag: Some(obj.etag),
                })
            }
            None => Ok(CurrentState {
                snapshot: Snapshot::empty(),
                etag: None,
            }),
        }
    }

    /// Install `bytes` into the slot: `If-Match` when advancing a known version, or `If-None-Match`
    /// to create the genesis slot.  Both are the same conditional-PUT primitive (§7).
    async fn install(&self, bytes: Vec<u8>, etag: &Option<String>) -> Result<WriteOutcome> {
        match etag {
            Some(e) => self.store.put_if_match(&self.slot_path, bytes, e).await,
            None => self.store.put_if_none_match(&self.slot_path, bytes).await,
        }
    }

    /// Convenience: write a single key/value at a client-supplied `timestamp`.  The timestamp must
    /// be strictly greater than the highest timestamp the database has incorporated; `write`
    /// enforces this and returns an `invalid-timestamp` [`SError`] otherwise.
    pub async fn put(
        &self,
        key: impl Into<Vec<u8>>,
        value: impl Into<Vec<u8>>,
        timestamp: u64,
    ) -> Result<LogPosition> {
        let key = key.into();
        let value = value.into();
        let mut batch = WriteBatch::new();
        batch.put(&key, timestamp, &value)?;
        self.write(&batch).await
    }

    /// Convenience: delete a single key at a client-supplied `timestamp`.  The timestamp must be
    /// strictly greater than the highest timestamp the database has incorporated; `write` enforces
    /// this and returns an `invalid-timestamp` [`SError`] otherwise.
    pub async fn delete(&self, key: impl Into<Vec<u8>>, timestamp: u64) -> Result<LogPosition> {
        let key = key.into();
        let mut batch = WriteBatch::new();
        batch.del(&key, timestamp)?;
        self.write(&batch).await
    }

    /// The §6 write loop, end to end: exact pre-claim capacity check, fragment claim
    /// (`If-None-Match`), rollup, and install (`If-Match`).  On any precondition collision it rolls
    /// the SST forward (§6.1) and retries.
    ///
    /// `write` carries a fixed batch, so re-applying it is idempotent; the operation
    /// linearizes exactly once at the install that incorporates its offset, regardless of who
    /// performs that install.
    pub async fn write(&self, batch: &WriteBatch) -> Result<LogPosition> {
        check_batch(batch)?;
        loop {
            let current = self.load_current().await?;
            let offset = current.snapshot.next_offset();

            // Step 0: exact, pre-claim capacity check (§9).  Gates the claim so we never make an
            // over-cap fragment durable.
            let new_snapshot = current.snapshot.apply(batch, offset)?;
            let bytes = new_snapshot.to_sst_bytes()?;
            if bytes.len() > self.options.cap {
                return Err(capacity_exceeded(bytes.len(), self.options.cap));
            }

            // Step 1: claim the offset durably (If-None-Match).
            let frag = encode(batch)?;
            match self
                .store
                .put_if_none_match(&self.fragment_path(offset), frag)
                .await?
            {
                WriteOutcome::Written(_) => {}
                WriteOutcome::PreconditionFailed => {
                    // Someone already owns this offset; they may have crashed before installing.
                    self.recover().await?;
                    continue;
                }
            }

            // --- fragment is now durable.  The operation is PENDING. ---

            // Step 2: roll the SST forward over the fragment we just claimed and install it.
            match self.install(bytes, &current.etag).await? {
                WriteOutcome::Written(_) => {
                    // --- SST now incorporates the offset.  The operation is LINEARIZED. ---
                    return Ok(LogPosition { offset });
                }
                WriteOutcome::PreconditionFailed => {
                    // The slot moved underneath us.  The only claimable next offset was ours, so
                    // whoever advanced the slot must have rolled our fragment forward — our write
                    // is already linearized at `offset`.  Confirm by reloading; if for any reason
                    // it is not yet incorporated, retry the loop.
                    self.recover().await?;
                    let after = self.load_current().await?;
                    if after.snapshot.log_hi() >= offset {
                        return Ok(LogPosition { offset });
                    }
                    continue;
                }
            }
        }
    }

    // ----------------------------------------------------------------------------- recovery

    /// Writer-driven roll-forward recovery (§6.1).  Brings the SST up to the durable log: while the
    /// fragment at `log_hi + 1` exists, apply it and install the result, verifying the multiset is
    /// exactly what the log says (§8).  `If-Match` collisions are ignored — another writer advanced
    /// it; we re-observe and proceed.  Terminates when the SST is caught up to the log.
    pub async fn recover(&self) -> Result<()> {
        loop {
            let current = self.load_current().await?;
            let next = current.snapshot.next_offset();
            let frag_obj = match self.store.get(&self.fragment_path(next)).await? {
                Some(obj) => obj,
                None => return Ok(()), // SST is caught up to the log.
            };
            let batch = decode(&frag_obj.bytes)?;
            let new_snapshot = current.snapshot.apply(&batch, next)?;

            // Turn roll-forward from "trust the offset pointer" into "verify the multiset is
            // exactly what the log says it should be" (§8): the incrementally-maintained setsum must
            // agree with a full recompute over the resulting user keyspace.
            let stamped = new_snapshot.setsum().hexdigest();
            let computed = new_snapshot.recompute_setsum().hexdigest();
            if stamped != computed {
                return Err(setsum_mismatch(&stamped, &computed));
            }

            let bytes = new_snapshot.to_sst_bytes()?;
            // Ignore PreconditionFailed: someone else advanced it; loop and re-observe.
            let _ = self.install(bytes, &current.etag).await?;
        }
    }

    // ----------------------------------------------------------------------------- transactions

    /// Begin an optimistic transaction.  Reads are served from the SST snapshot captured here, and
    /// the captured ETag is the transaction's read-version (§7).
    pub async fn transaction(&self) -> Result<Transaction> {
        let current = self.load_current().await?;
        Ok(Transaction {
            db: self.clone(),
            snapshot: Arc::new(current.snapshot),
            etag: current.etag,
            batch: WriteBatch::new(),
        })
    }
}

/// A single-object optimistic, serializable transaction (§7).
///
/// The ETag captured at [`Database::transaction`] *is* the read-version: the transaction commits
/// iff no other writer has advanced the SST since it read (first-committer-wins).  On conflict,
/// [`Transaction::commit`] returns a `conflict` [`SError`] and the caller should re-read, reapply its
/// read-modify-write logic, and retry.
pub struct Transaction {
    db: Database,
    snapshot: Arc<Snapshot>,
    etag: Option<String>,
    batch: WriteBatch,
}

impl Transaction {
    /// The snapshot this transaction reads from.  All reads in the transaction are served from this
    /// single version.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Read a key at the transaction's snapshot.
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        self.snapshot.get(key)
    }

    /// Stage a put at a client-supplied `timestamp`.  The timestamp must be strictly greater than
    /// the highest timestamp incorporated at the transaction's read-version; `commit` enforces this
    /// (against the captured snapshot's high-water mark) and returns an `invalid-timestamp`
    /// [`SError`] otherwise.
    pub fn put(
        &mut self,
        key: impl Into<Vec<u8>>,
        value: impl Into<Vec<u8>>,
        timestamp: u64,
    ) -> Result<&mut Self> {
        let key = key.into();
        let value = value.into();
        self.batch.put(&key, timestamp, &value)?;
        Ok(self)
    }

    /// Stage a delete at a client-supplied `timestamp`.  The timestamp must be strictly greater
    /// than the highest timestamp incorporated at the transaction's read-version; `commit` enforces
    /// this and returns an `invalid-timestamp` [`SError`] otherwise.
    pub fn delete(&mut self, key: impl Into<Vec<u8>>, timestamp: u64) -> Result<&mut Self> {
        let key = key.into();
        self.batch.del(&key, timestamp)?;
        Ok(self)
    }

    /// Commit by advancing the tag.  A single first-committer-wins attempt against the captured
    /// read-version: success serializes the transaction immediately after the version it read;
    /// failure returns a `conflict` [`SError`].
    ///
    /// A transaction that stages no writes is a read-only transaction and commits trivially
    /// without writing a fragment.
    pub async fn commit(self) -> Result<LogPosition> {
        check_batch(&self.batch)?;

        if self.batch.is_empty() {
            return Ok(LogPosition {
                offset: self.snapshot.log_hi(),
            });
        }

        let offset = self.snapshot.next_offset();
        let new_snapshot = self.snapshot.apply(&self.batch, offset)?;
        let bytes = new_snapshot.to_sst_bytes()?;
        if bytes.len() > self.db.options.cap {
            return Err(capacity_exceeded(bytes.len(), self.db.options.cap));
        }

        // Claim the offset.  If we lose the claim, a conflicting transaction got there first.
        let frag = encode(&self.batch)?;
        match self
            .db
            .store
            .put_if_none_match(&self.db.fragment_path(offset), frag)
            .await?
        {
            WriteOutcome::Written(_) => {}
            WriteOutcome::PreconditionFailed => {
                self.db.recover().await?;
                return Err(conflict());
            }
        }

        // Commit by advancing the tag.
        match self.db.install(bytes, &self.etag).await? {
            WriteOutcome::Written(_) => Ok(LogPosition { offset }),
            WriteOutcome::PreconditionFailed => {
                // The slot moved.  The only claimable next offset was ours, so whoever advanced it
                // rolled our fragment forward — our transaction committed at `offset`.  Confirm.
                self.db.recover().await?;
                let after = self.db.load_current().await?;
                if after.snapshot.log_hi() >= offset {
                    Ok(LogPosition { offset })
                } else {
                    Err(conflict())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ObjectStoreConditional;

    fn db() -> Database {
        Database::open(
            Arc::new(ObjectStoreConditional::in_memory()),
            DatabaseOptions {
                window: Duration::from_millis(0),
                ..DatabaseOptions::default()
            },
        )
    }

    /// The full S-expression [`handled::SError`] emits for an `invalid-timestamp` rejection, with
    /// `ts` the offending entry's timestamp and `hwm` the high-water mark it failed to beat.  We
    /// assert the exact serialized form rather than a substring, so a wrong code or field shows up
    /// as a concrete diff.
    fn invalid_timestamp_sexp(ts: u64, hwm: u64) -> String {
        format!(
            "(error (phase sstdb) (code invalid-timestamp) \
             (message \"sstdb write batches must use strictly-increasing timestamps\") \
             (timestamp {ts}) (high-water-mark {hwm}))"
        )
    }

    #[tokio::test]
    async fn put_enforces_strictly_increasing_timestamps() {
        let db = db();
        // The genesis high-water mark is 0, so timestamp 0 is rejected.
        let err = db.put(b"k", b"v", 0).await.unwrap_err();
        assert_eq!(format!("{err}"), invalid_timestamp_sexp(0, 0));
        // A strictly-increasing sequence is accepted and readable.
        let p1 = db.put(b"k", b"v1", 1).await.unwrap();
        assert_eq!(p1.offset, 1);
        assert_eq!(db.get(b"k").await.unwrap(), Some(b"v1".to_vec()));
        let p2 = db.put(b"k", b"v2", 2).await.unwrap();
        assert_eq!(p2.offset, 2);
        assert_eq!(db.get(b"k").await.unwrap(), Some(b"v2".to_vec()));
        assert_eq!(db.snapshot().await.unwrap().timestamp_hi(), 2);
        // Reusing the last timestamp is rejected.
        let err = db.put(b"k", b"v3", 2).await.unwrap_err();
        assert_eq!(format!("{err}"), invalid_timestamp_sexp(2, 2));
        // Going backwards is rejected.
        let err = db.put(b"k", b"v3", 1).await.unwrap_err();
        assert_eq!(format!("{err}"), invalid_timestamp_sexp(1, 2));
    }

    #[tokio::test]
    async fn delete_carries_a_timestamp() {
        let db = db();
        db.put(b"k", b"v", 1).await.unwrap();
        assert_eq!(db.get(b"k").await.unwrap(), Some(b"v".to_vec()));
        db.delete(b"k", 2).await.unwrap();
        assert_eq!(db.get(b"k").await.unwrap(), None);
        assert_eq!(db.snapshot().await.unwrap().timestamp_hi(), 2);
    }
}

