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
use sst::{Builder, Cursor, Key, Sst, SstBuilder, SstOptions};

use crate::fragment::{SSTDB_TIMESTAMP, is_reserved_key};
use crate::{
    META_PREFIX_LEN, Result, corruption, invalid_timestamp, reserved_key, setsum_mismatch,
};

/// The reserved meta-key prefix: any key beginning with these bytes is a meta key.
pub const META_PREFIX: [u8; META_PREFIX_LEN] = [0xff; META_PREFIX_LEN];

const META_LOG_LO: &[u8] = b"/log_lo";
const META_LOG_HI: &[u8] = b"/log_hi";
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
/// boundary ambiguity from aliasing distinct pairs into the same multiset element.
fn setsum_item(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut item = Vec::with_capacity(8 + key.len() + value.len());
    item.extend_from_slice(&(key.len() as u64).to_le_bytes());
    item.extend_from_slice(key);
    item.extend_from_slice(value);
    item
}

fn snapshot_key(key: &[u8]) -> Key {
    Key {
        key: key.to_vec(),
        timestamp: SSTDB_TIMESTAMP,
    }
}

/// The materialized database: the user keyspace plus the log metadata.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// The live user keyspace, sorted.  Tombstones are not represented — a deleted key is simply
    /// absent.
    map: BTreeMap<Key, Vec<u8>>,
    /// Lowest fragment offset still represented (0 for v1 without GC).
    log_lo: u64,
    /// Highest fragment offset incorporated.  The recovery anchor.
    log_hi: u64,
    /// The setsum over the user keyspace.  Maintained incrementally by [`Snapshot::apply`].
    setsum: Setsum,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self::empty()
    }
}

impl Snapshot {
    /// The genesis (empty) database: no user keys, `log_lo == log_hi == 0`.
    pub fn empty() -> Self {
        Snapshot {
            map: BTreeMap::new(),
            log_lo: 0,
            log_hi: 0,
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
        self.map.get(&snapshot_key(key)).map(|v| v.as_slice())
    }

    /// Range scan over the user keyspace, returning owned key/value pairs in sorted order.  Meta
    /// keys are excluded.
    pub fn scan<S, E>(&self, start: Bound<S>, end: Bound<E>) -> Vec<(Vec<u8>, Vec<u8>)>
    where
        S: AsRef<[u8]>,
        E: AsRef<[u8]>,
    {
        let start = match &start {
            Bound::Included(s) => Bound::Included(snapshot_key(s.as_ref())),
            Bound::Excluded(s) => Bound::Excluded(snapshot_key(s.as_ref())),
            Bound::Unbounded => Bound::Unbounded,
        };
        let end = match &end {
            Bound::Included(e) => Bound::Included(snapshot_key(e.as_ref())),
            Bound::Excluded(e) => Bound::Excluded(snapshot_key(e.as_ref())),
            Bound::Unbounded => Bound::Unbounded,
        };
        self.map
            .range((start, end))
            .filter(|(k, _)| !is_reserved_key(&k.key))
            .map(|(k, v)| (k.key.clone(), v.clone()))
            .collect()
    }

    /// Iterate over all live user key/value pairs in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.map
            .iter()
            .map(|(k, v)| (k.key.as_slice(), v.as_slice()))
    }

    /// Produce a new snapshot with `batch` applied and `log_hi` advanced to `offset`.
    ///
    /// This is the pure, deterministic rollup `apply(SST, fragment)` (Invariant 4).  The setsum is
    /// maintained incrementally: `setsum' = setsum + additions - removals` (§8).
    pub fn apply(&self, batch: &WriteBatch, offset: u64) -> Result<Snapshot> {
        let mut map = self.map.clone();
        let mut setsum = self.setsum;
        let mut iter = batch.iter();
        while let Some(kvr) = iter.next()? {
            if kvr.timestamp != SSTDB_TIMESTAMP {
                return Err(invalid_timestamp(kvr.timestamp));
            }
            if is_reserved_key(kvr.key) {
                return Err(reserved_key());
            }
            let key = snapshot_key(kvr.key);
            match kvr.value {
                Some(value) => {
                    if let Some(old) = map.insert(key.clone(), value.to_vec()) {
                        setsum.remove(&setsum_item(&key.key, &old));
                    }
                    setsum.insert(&setsum_item(&key.key, value));
                }
                None => {
                    if let Some(old) = map.remove(&key) {
                        setsum.remove(&setsum_item(&key.key, &old));
                    }
                }
            }
        }
        Ok(Snapshot {
            map,
            log_lo: self.log_lo,
            log_hi: offset,
            setsum,
        })
    }

    /// Recompute the setsum from scratch over the user keyspace.  Used by writers to verify a
    /// loaded SST before building on it (§8).
    pub fn recompute_setsum(&self) -> Setsum {
        let mut s = Setsum::default();
        for (k, v) in &self.map {
            s.insert(&setsum_item(&k.key, v));
        }
        s
    }

    /// Serialize this snapshot to a real SST's bytes.  The meta keys are stamped from the
    /// snapshot's own `log_lo`/`log_hi`/`setsum` (§8: stamping the setsum does not perturb it,
    /// because meta keys are excluded from the setsum domain).
    pub fn to_sst_bytes(&self) -> Result<Vec<u8>> {
        let mut builder = SstBuilder::<Vec<u8>>::from_write(sst_options(), Vec::new());
        // User keys first (already sorted), all at timestamp 0 for a single materialized version.
        for (k, v) in &self.map {
            builder.put(&k.key, k.timestamp, v)?;
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
        let mut stamped_setsum: Option<String> = None;

        while let Some(kvr) = cursor.key_value() {
            let key = kvr.key;
            if kvr.timestamp != SSTDB_TIMESTAMP {
                return Err(invalid_timestamp(kvr.timestamp));
            }
            if is_reserved_key(key) {
                let suffix = &key[META_PREFIX_LEN..];
                let value = kvr.value.unwrap_or(&[]);
                if suffix == META_LOG_LO {
                    log_lo = Some(parse_hex_u64(value)?);
                } else if suffix == META_LOG_HI {
                    log_hi = Some(parse_hex_u64(value)?);
                } else if suffix == META_SETSUM {
                    stamped_setsum = Some(
                        std::str::from_utf8(value)
                            .map_err(|_| corruption("meta: setsum not utf8"))?
                            .to_string(),
                    );
                }
                // Unknown meta keys are ignored for forward compatibility.
            } else if let Some(v) = kvr.value {
                map.insert(snapshot_key(key), v.to_vec());
            }
            cursor.next()?;
        }

        let log_lo = log_lo.ok_or_else(|| corruption("meta: missing log_lo"))?;
        let log_hi = log_hi.ok_or_else(|| corruption("meta: missing log_hi"))?;
        let stamped = stamped_setsum.ok_or_else(|| corruption("meta: missing setsum"))?;

        let snapshot = Snapshot {
            map,
            log_lo,
            log_hi,
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
        assert!(back.is_empty());
        assert_eq!(back.setsum().hexdigest(), Setsum::default().hexdigest());
    }

    #[test]
    fn apply_and_round_trip() {
        let mut batch = WriteBatch::new();
        batch.put(b"alpha", SSTDB_TIMESTAMP, b"1").unwrap();
        batch.put(b"beta", SSTDB_TIMESTAMP, b"2").unwrap();
        let snap = Snapshot::empty().apply(&batch, 1).unwrap();
        assert_eq!(snap.log_hi(), 1);
        assert_eq!(snap.get(b"alpha"), Some(&b"1"[..]));

        let bytes = snap.to_sst_bytes().unwrap();
        let back = Snapshot::from_sst_bytes(&bytes, true).unwrap();
        assert_eq!(back.get(b"alpha"), Some(&b"1"[..]));
        assert_eq!(back.get(b"beta"), Some(&b"2"[..]));
        assert_eq!(back.log_hi(), 1);
        // The stamped setsum survived the round trip and verifies.
        assert_eq!(back.setsum().hexdigest(), snap.setsum().hexdigest());
    }

    #[test]
    fn incremental_setsum_matches_recompute() {
        let mut batch = WriteBatch::new();
        batch.put(b"a", SSTDB_TIMESTAMP, b"x").unwrap();
        batch.put(b"b", SSTDB_TIMESTAMP, b"y").unwrap();
        batch.put(b"a", SSTDB_TIMESTAMP, b"z").unwrap(); // overwrite
        batch.del(b"b", SSTDB_TIMESTAMP).unwrap();
        let snap = Snapshot::empty().apply(&batch, 1).unwrap();
        assert_eq!(snap.get(b"a"), Some(&b"z"[..]));
        assert_eq!(snap.get(b"b"), None);
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
}
