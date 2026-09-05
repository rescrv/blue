//! The [`Snapshot`]: the materialized database.
//!
//! The entire live dataset is one SST (Invariant 1).  In memory we keep it as a sorted map of the
//! user keyspace plus three pieces of metadata (`log_lo`, `log_hi`, `setsum`).  On object storage
//! it is a genuine SST produced by the `sst` crate, with the metadata stored *in-keyspace* as
//! reserved meta keys (§4.1):
//!
//! * `\xff\xff\xff\xff\xff/log_lo`  — lowest fragment offset still represented in this SST.
//! * `\xff\xff\xff\xff\xff/log_hi`  — highest fragment offset incorporated into this SST.
//! * `\xff\xff\xff\xff\xff/setsum`  — the setsum of the *user* keyspace (meta keys excluded).
//!
//! Because `0xff`×5 sorts after all plausible user keys, the meta keys live at the tail of the SST
//! and are trivially separable on scan.
//!
//! Offsets follow the §6 pseudocode: `log_hi` is the highest incorporated offset and the next
//! offset to claim is `log_hi + 1`.  The genesis (empty) database has `log_hi == 0`; fragment 0 is
//! the implicit empty genesis and is never written as an object, so the first real write claims
//! offset 1.

use std::collections::BTreeMap;
use std::ops::Bound;

use setsum::Setsum;
use sst::file_manager::InMemoryFile;
use sst::log::WriteBatch;
use sst::{Builder, Cursor, Sst, SstBuilder, SstOptions};

use crate::fragment::is_reserved_key;
use crate::{
    META_PREFIX_LEN, Result, corruption, invalid_timestamp, reserved_key, setsum_mismatch,
};

/// The reserved meta-key prefix: any key beginning with these bytes is a meta key.
pub const META_PREFIX: [u8; META_PREFIX_LEN] = [0xff; META_PREFIX_LEN];

const META_LOG_LO: &[u8] = b"/log_lo";
const META_LOG_HI: &[u8] = b"/log_hi";
const META_TIMESTAMP_HI: &[u8] = b"/timestamp_hi";
const META_SETSUM: &[u8] = b"/setsum";

/// All SSTs in sstdb share one fixed option set so that `apply` is byte-deterministic across
/// writers (Invariant 4).
fn sst_options() -> SstOptions {
    SstOptions::default()
}

/// Build the full meta key for a given suffix.
fn meta_key(suffix: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(META_PREFIX_LEN + suffix.len());
    k.extend_from_slice(&META_PREFIX);
    k.extend_from_slice(suffix);
    k
}

/// The framed setsum item for a single key/value pair.  Framing the key length prevents a key/value
/// boundary ambiguity from aliasing distinct pairs into the same multiset element.  The setsum is
/// over the materialized user keyspace (key, value); the timestamp is an ordering token, not part
/// of the materialized identity, so it is deliberately excluded.
fn setsum_item(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut item = Vec::with_capacity(8 + key.len() + value.len());
    item.extend_from_slice(&(key.len() as u64).to_le_bytes());
    item.extend_from_slice(key);
    item.extend_from_slice(value);
    item
}

/// The materialized database: the user keyspace plus the log metadata.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// The live user keyspace, sorted by key.  Each entry carries the timestamp of the write that
    /// last touched it; sstdb materializes exactly one version per key (the latest write wins), so
    /// tombstones are not represented — a deleted key is simply absent.
    map: BTreeMap<Vec<u8>, (u64, Vec<u8>)>,
    /// Lowest fragment offset still represented (0 for v1 without GC).
    log_lo: u64,
    /// Highest fragment offset incorporated.  The recovery anchor.
    log_hi: u64,
    /// The highest timestamp incorporated.  The strictly-increasing high-water mark: every entry
    /// of the next write must carry a timestamp greater than this.
    timestamp_hi: u64,
    /// The setsum over the user keyspace.  Maintained incrementally by [`Snapshot::apply`].
    setsum: Setsum,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self::empty()
    }
}

impl Snapshot {
    /// The genesis (empty) database: no user keys, `log_lo == log_hi == 0`, and a timestamp
    /// high-water mark of 0 (so the first write must use a timestamp strictly greater than 0).
    pub fn empty() -> Self {
        Snapshot {
            map: BTreeMap::new(),
            log_lo: 0,
            log_hi: 0,
            timestamp_hi: 0,
            setsum: Setsum::default(),
        }
    }

    /// Lowest fragment offset still represented.
    pub fn log_lo(&self) -> u64 {
        self.log_lo
    }

    /// Highest fragment offset incorporated.  The next offset to claim is `log_hi() + 1`.
    pub fn log_hi(&self) -> u64 {
        self.log_hi
    }

    /// The highest timestamp incorporated: the strictly-increasing high-water mark.  Every entry of
    /// the next write must carry a timestamp greater than this value.
    pub fn timestamp_hi(&self) -> u64 {
        self.timestamp_hi
    }

    /// The next offset a writer would claim.
    pub fn next_offset(&self) -> u64 {
        self.log_hi + 1
    }

    /// The setsum over the user keyspace.
    pub fn setsum(&self) -> Setsum {
        self.setsum
    }

    /// Number of live user keys.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if there are no live user keys.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Point lookup over the user keyspace.  Meta keys are never visible to clients.
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        if is_reserved_key(key) {
            return None;
        }
        self.map.get(key).map(|(_, v)| v.as_slice())
    }

    /// Range scan over the user keyspace, returning owned key/value pairs in sorted order.  Meta
    /// keys are excluded.
    pub fn scan<S, E>(&self, start: Bound<S>, end: Bound<E>) -> Vec<(Vec<u8>, Vec<u8>)>
    where
        S: AsRef<[u8]>,
        E: AsRef<[u8]>,
    {
        let start = match &start {
            Bound::Included(s) => Bound::Included(s.as_ref().to_vec()),
            Bound::Excluded(s) => Bound::Excluded(s.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };
        let end = match &end {
            Bound::Included(e) => Bound::Included(e.as_ref().to_vec()),
            Bound::Excluded(e) => Bound::Excluded(e.as_ref().to_vec()),
            Bound::Unbounded => Bound::Unbounded,
        };
        self.map
            .range((start, end))
            .filter(|(k, _)| !is_reserved_key(k))
            .map(|(k, (_, v))| (k.clone(), v.clone()))
            .collect()
    }

    /// Iterate over all live user key/value pairs in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.map
            .iter()
            .map(|(k, (_, v))| (k.as_slice(), v.as_slice()))
    }

    /// Produce a new snapshot with `batch` applied and `log_hi` advanced to `offset`.
    ///
    /// This is the pure, deterministic rollup `apply(SST, fragment)` (Invariant 4).  The setsum is
    /// maintained incrementally: `setsum' = setsum + additions - removals` (§8).
    ///
    /// The client-supplied timestamps are enforced here, against the snapshot's high-water mark:
    /// every entry of `batch` must carry a timestamp strictly greater than the last timestamp the
    /// database incorporated (and strictly greater than the preceding entry of the same batch), so
    /// the incorporated timestamps form a strictly increasing total order.  The high-water mark is
    /// advanced to the batch's final timestamp.
    pub fn apply(&self, batch: &WriteBatch, offset: u64) -> Result<Snapshot> {
        let mut map = self.map.clone();
        let mut setsum = self.setsum;
        let mut high_water_mark = self.timestamp_hi;
        let mut iter = batch.iter();
        while let Some(kvr) = iter.next()? {
            if kvr.timestamp <= high_water_mark {
                return Err(invalid_timestamp(kvr.timestamp, high_water_mark));
            }
            if is_reserved_key(kvr.key) {
                return Err(reserved_key());
            }
            high_water_mark = kvr.timestamp;
            match kvr.value {
                Some(value) => {
                    if let Some((_, old)) =
                        map.insert(kvr.key.to_vec(), (kvr.timestamp, value.to_vec()))
                    {
                        setsum.remove(&setsum_item(kvr.key, &old));
                    }
                    setsum.insert(&setsum_item(kvr.key, value));
                }
                None => {
                    if let Some((_, old)) = map.remove(kvr.key) {
                        setsum.remove(&setsum_item(kvr.key, &old));
                    }
                }
            }
        }
        Ok(Snapshot {
            map,
            log_lo: self.log_lo,
            log_hi: offset,
            timestamp_hi: high_water_mark,
            setsum,
        })
    }

    /// Recompute the setsum from scratch over the user keyspace.  Used by writers to verify a
    /// loaded SST before building on it (§8).
    pub fn recompute_setsum(&self) -> Setsum {
        let mut s = Setsum::default();
        for (k, (_, v)) in &self.map {
            s.insert(&setsum_item(k, v));
        }
        s
    }

    /// Serialize this snapshot to a real SST's bytes.  The meta keys are stamped from the
    /// snapshot's own `log_lo`/`log_hi`/`timestamp_hi`/`setsum` (§8: stamping the setsum does not
    /// perturb it, because meta keys are excluded from the setsum domain).
    pub fn to_sst_bytes(&self) -> Result<Vec<u8>> {
        let mut builder = SstBuilder::<Vec<u8>>::from_write(sst_options(), Vec::new());
        // User keys first (already sorted), each stamped with the timestamp of the write that
        // last touched it.
        for (k, (timestamp, v)) in &self.map {
            builder.put(k, *timestamp, v)?;
        }
        // Meta keys sort after all user keys; emit them in sorted order among themselves.
        let mut metas: Vec<(Vec<u8>, Vec<u8>)> = vec![
            (
                meta_key(META_LOG_LO),
                format!("{:016x}", self.log_lo).into_bytes(),
            ),
            (
                meta_key(META_LOG_HI),
                format!("{:016x}", self.log_hi).into_bytes(),
            ),
            (
                meta_key(META_TIMESTAMP_HI),
                format!("{:016x}", self.timestamp_hi).into_bytes(),
            ),
            (meta_key(META_SETSUM), self.setsum.hexdigest().into_bytes()),
        ];
        metas.sort();
        for (k, v) in &metas {
            builder.put(k, 0, v)?;
        }
        let bytes = builder.seal()?;
        Ok(bytes)
    }

    /// Parse an SST's bytes back into a snapshot, separating meta keys from the user keyspace.
    ///
    /// If `verify` is set, the recomputed setsum over the user keyspace is checked against the
    /// stamped setsum and a `setsum-mismatch` [`SError`] is returned on disagreement.  Writers verify
    /// on load; readers trust the stamp by default (§8).
    pub fn from_sst_bytes(bytes: &[u8], verify: bool) -> Result<Snapshot> {
        let sst = Sst::<InMemoryFile>::from_bytes(bytes.to_vec())?;
        Self::from_sst(&sst, verify)
    }

    fn from_sst(sst: &Sst<InMemoryFile>, verify: bool) -> Result<Snapshot> {
        let mut cursor = sst.cursor();
        cursor.seek_to_first()?;
        cursor.next()?;

        let mut map = BTreeMap::new();
        let mut log_lo: Option<u64> = None;
        let mut log_hi: Option<u64> = None;
        let mut timestamp_hi: Option<u64> = None;
        let mut stamped_setsum: Option<String> = None;

        while let Some(kvr) = cursor.key_value() {
            let key = kvr.key;
            if is_reserved_key(key) {
                let suffix = &key[META_PREFIX_LEN..];
                let value = kvr.value.unwrap_or(&[]);
                if suffix == META_LOG_LO {
                    log_lo = Some(parse_hex_u64(value)?);
                } else if suffix == META_LOG_HI {
                    log_hi = Some(parse_hex_u64(value)?);
                } else if suffix == META_TIMESTAMP_HI {
                    timestamp_hi = Some(parse_hex_u64(value)?);
                } else if suffix == META_SETSUM {
                    stamped_setsum = Some(
                        std::str::from_utf8(value)
                            .map_err(|_| corruption("meta: setsum not utf8"))?
                            .to_string(),
                    );
                }
                // Unknown meta keys are ignored for forward compatibility.
            } else if let Some(v) = kvr.value {
                map.insert(key.to_vec(), (kvr.timestamp, v.to_vec()));
            }
            cursor.next()?;
        }

        let log_lo = log_lo.ok_or_else(|| corruption("meta: missing log_lo"))?;
        let log_hi = log_hi.ok_or_else(|| corruption("meta: missing log_hi"))?;
        // SSTs written before timestamps were tracked carry no `timestamp_hi` meta key; default to
        // 0, the baseline from which strictly-increasing timestamps begin.
        let timestamp_hi = timestamp_hi.unwrap_or(0);
        let stamped = stamped_setsum.ok_or_else(|| corruption("meta: missing setsum"))?;

        let snapshot = Snapshot {
            map,
            log_lo,
            log_hi,
            timestamp_hi,
            setsum: Setsum::from_hexdigest(&stamped)
                .ok_or_else(|| corruption("meta: setsum not valid hex"))?,
        };

        if verify {
            let computed = snapshot.recompute_setsum().hexdigest();
            if computed != stamped {
                return Err(setsum_mismatch(&stamped, &computed));
            }
        }

        Ok(snapshot)
    }
}

fn parse_hex_u64(bytes: &[u8]) -> Result<u64> {
    let s = std::str::from_utf8(bytes).map_err(|_| corruption("meta: offset not utf8"))?;
    u64::from_str_radix(s, 16).map_err(|_| corruption("meta: offset not hex"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_round_trip() {
        let snap = Snapshot::empty();
        let bytes = snap.to_sst_bytes().unwrap();
        let back = Snapshot::from_sst_bytes(&bytes, true).unwrap();
        assert_eq!(back.log_lo(), 0);
        assert_eq!(back.log_hi(), 0);
        assert_eq!(back.timestamp_hi(), 0);
        assert!(back.is_empty());
        assert_eq!(back.setsum().hexdigest(), Setsum::default().hexdigest());
    }

    #[test]
    fn apply_and_round_trip() {
        let mut batch = WriteBatch::new();
        batch.put(b"alpha", 1, b"1").unwrap();
        batch.put(b"beta", 2, b"2").unwrap();
        let snap = Snapshot::empty().apply(&batch, 1).unwrap();
        assert_eq!(snap.log_hi(), 1);
        assert_eq!(snap.timestamp_hi(), 2);
        assert_eq!(snap.get(b"alpha"), Some(&b"1"[..]));

        let bytes = snap.to_sst_bytes().unwrap();
        let back = Snapshot::from_sst_bytes(&bytes, true).unwrap();
        assert_eq!(back.get(b"alpha"), Some(&b"1"[..]));
        assert_eq!(back.get(b"beta"), Some(&b"2"[..]));
        assert_eq!(back.log_hi(), 1);
        assert_eq!(back.timestamp_hi(), 2);
        // The stamped setsum survived the round trip and verifies.
        assert_eq!(back.setsum().hexdigest(), snap.setsum().hexdigest());
    }

    #[test]
    fn incremental_setsum_matches_recompute() {
        let mut batch = WriteBatch::new();
        batch.put(b"a", 1, b"x").unwrap();
        batch.put(b"b", 2, b"y").unwrap();
        batch.put(b"a", 3, b"z").unwrap(); // overwrite
        batch.del(b"b", 4).unwrap();
        let snap = Snapshot::empty().apply(&batch, 1).unwrap();
        assert_eq!(snap.get(b"a"), Some(&b"z"[..]));
        assert_eq!(snap.get(b"b"), None);
        assert_eq!(snap.timestamp_hi(), 4);
        assert_eq!(
            snap.setsum().hexdigest(),
            snap.recompute_setsum().hexdigest()
        );
    }

    #[test]
    fn meta_keys_not_visible() {
        let snap = Snapshot::empty().apply(&WriteBatch::new(), 5).unwrap();
        // No user key equals a meta key.
        assert_eq!(snap.get(&META_PREFIX), None);
        let all = snap.scan::<&[u8], &[u8]>(Bound::Unbounded, Bound::Unbounded);
        assert!(all.is_empty());
    }

    /// The database enforces strictly-increasing timestamps.  A batch whose first timestamp does
    /// not exceed the genesis high-water mark (0) is rejected, as is a batch that repeats or
    /// decreases a timestamp within itself.
    #[test]
    fn strictly_increasing_enforced() {
        // Timestamp 0 is not strictly greater than the genesis high-water mark 0.
        let mut batch = WriteBatch::new();
        batch.put(b"a", 0, b"x").unwrap();
        assert!(Snapshot::empty().apply(&batch, 1).is_err());

        // A batch must be internally strictly increasing too.
        let mut batch = WriteBatch::new();
        batch.put(b"a", 1, b"x").unwrap();
        batch.put(b"b", 1, b"y").unwrap();
        assert!(Snapshot::empty().apply(&batch, 1).is_err());

        // A later batch must exceed the prior high-water mark.
        let mut first = WriteBatch::new();
        first.put(b"a", 5, b"x").unwrap();
        let snap = Snapshot::empty().apply(&first, 1).unwrap();
        assert_eq!(snap.timestamp_hi(), 5);
        let mut second = WriteBatch::new();
        second.put(b"b", 5, b"y").unwrap(); // not strictly greater than 5
        assert!(snap.apply(&second, 2).is_err());
        let mut third = WriteBatch::new();
        third.put(b"b", 6, b"y").unwrap(); // strictly greater than 5
        let snap2 = snap.apply(&third, 2).unwrap();
        assert_eq!(snap2.timestamp_hi(), 6);
        assert_eq!(snap2.get(b"a"), Some(&b"x"[..]));
        assert_eq!(snap2.get(b"b"), Some(&b"y"[..]));
    }
}
