//! Fragments: the immutable, ordered log records.
//!
//! A fragment is a single immutable encoding of one write's [`WriteBatch`] (§4).  Once written, a
//! fragment is never mutated (Invariant 4), and `apply(SST, fragment)` is a pure, deterministic
//! function so that whoever installs the resulting SST computes the same bytes.
//!
//! A fragment is encoded as an `sst` log: the caller's [`WriteBatch`] is written through a
//! [`LogBuilder`] into an in-memory `Vec<u8>` — the bytes we upload as the fragment object.
//! Decoding replays the log with a [`LogIterator`] back into a [`WriteBatch`].  Reusing the `sst`
//! batch and log keeps a single, tested mutation model rather than a second hand-rolled one.

use sst::log::WriteBatch;
use sst::{Builder, KeyValueRef, LogBuilder, LogIterator, LogOptions};

use crate::{META_PREFIX_LEN, Result, invalid_timestamp, reserved_key};

/// sstdb materializes exactly one user-visible version in the SST.
pub(crate) const SSTDB_TIMESTAMP: u64 = 0;

/// Returns true if `key` is in the reserved meta-key range (begins with [`META_PREFIX_LEN`]
/// `0xff` bytes).
pub fn is_reserved_key(key: &[u8]) -> bool {
    key.len() >= META_PREFIX_LEN && key[..META_PREFIX_LEN].iter().all(|b| *b == 0xff)
}

fn check_entry(kvr: &KeyValueRef<'_>) -> Result<()> {
    if kvr.timestamp != SSTDB_TIMESTAMP {
        return Err(invalid_timestamp(kvr.timestamp));
    }
    if is_reserved_key(kvr.key) {
        return Err(reserved_key());
    }
    Ok(())
}

/// Reject the batch if any entry targets a reserved meta key or uses a non-zero timestamp.
pub(crate) fn check_batch(batch: &WriteBatch) -> Result<()> {
    let mut iter = batch.iter();
    while let Some(kvr) = iter.next()? {
        check_entry(&kvr)?;
    }
    Ok(())
}

/// Encode a batch into a fragment's bytes: an `sst` log holding one write batch.  An empty batch
/// encodes as an empty log (no batch is appended, because the log rejects empty batches).
pub(crate) fn encode(batch: &WriteBatch) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut log = LogBuilder::from_write(LogOptions::default(), &mut buf)?;
        if !batch.is_empty() {
            log.append(batch)?;
        }
        log.seal()?;
    }
    Ok(buf)
}

/// Decode a fragment's bytes back into a [`WriteBatch`] by replaying the `sst` log.
pub(crate) fn decode(bytes: &[u8]) -> Result<WriteBatch> {
    let mut iter = LogIterator::from_reader(LogOptions::default(), std::io::Cursor::new(bytes))?;
    let mut batch = WriteBatch::new();
    while let Some(kvr) = iter.next()? {
        check_entry(&kvr)?;
        batch.insert(kvr)?;
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut batch = WriteBatch::new();
        batch.put(b"a", SSTDB_TIMESTAMP, b"1").unwrap();
        batch.del(b"b", SSTDB_TIMESTAMP).unwrap();
        batch.put(b"c", SSTDB_TIMESTAMP, b"").unwrap();
        let bytes = encode(&batch).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(batch, back);
    }

    #[test]
    fn empty_round_trip() {
        let batch = WriteBatch::new();
        // An empty batch appends no batch, so the log seals to zero bytes.
        let bytes = encode(&batch).unwrap();
        assert!(bytes.is_empty());
        assert_eq!(batch, decode(&bytes).unwrap());
    }

    /// A small fragment seals to a small object: the sst log writes `header + batch` and only
    /// trues-up to the 1 MiB block boundary when a batch crosses one, so a single tiny write is
    /// not padded out to a block.
    #[test]
    fn small_fragment_is_compact() {
        let mut batch = WriteBatch::new();
        batch.put(b"a", SSTDB_TIMESTAMP, b"1").unwrap();
        let bytes = encode(&batch).unwrap();
        assert!(!bytes.is_empty());
        // Far below the 1 MiB block boundary: no block-sized padding crept in.
        assert!(
            bytes.len() < 1024,
            "fragment unexpectedly large: {} bytes",
            bytes.len()
        );
        assert_eq!(batch, decode(&bytes).unwrap());
    }

    #[test]
    fn reserved_detection() {
        let mut batch = WriteBatch::new();
        batch.put(&[0xff; 6], SSTDB_TIMESTAMP, b"x").unwrap();
        assert!(check_batch(&batch).is_err());
        let mut ok = WriteBatch::new();
        ok.put(&[0xff; 4], SSTDB_TIMESTAMP, b"x").unwrap();
        assert!(check_batch(&ok).is_ok());
    }

    #[test]
    fn timestamp_detection() {
        let mut batch = WriteBatch::new();
        batch.put(b"a", 1, b"x").unwrap();
        assert!(check_batch(&batch).is_err());
    }
}
