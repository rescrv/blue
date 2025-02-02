//! A Setsum type that's more ergonomic for working with key-value pairs.

use setsum::Setsum as RawSetsum;

pub use setsum::SETSUM_BYTES;

use super::KeyValueRef;

////////////////////////////////////////////// Setsum //////////////////////////////////////////////

/// A wrapper around the Setsum type that provides methods to uniformly insert KeyValueRef, or
/// put/del key-value pairs.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Setsum {
    setsum: RawSetsum,
}

impl Setsum {
    /// The digest of this setsum.
    pub fn digest(&self) -> [u8; 32] {
        self.setsum.digest()
    }

    /// Build this setsum from a digest.
    pub fn from_digest(digest: [u8; SETSUM_BYTES]) -> Setsum {
        let setsum = RawSetsum::from_digest(digest);
        Self { setsum }
    }

    /// The hexdigest of this setsum.
    pub fn hexdigest(&self) -> String {
        self.setsum.hexdigest()
    }

    /// Build this setsum from a hexdigest.
    pub fn from_hexdigest(digest: &str) -> Option<Setsum> {
        let setsum = RawSetsum::from_hexdigest(digest)?;
        Some(Setsum { setsum })
    }

    /// Insert the KeyValueRef into the setsum.
    pub fn insert(&mut self, kvr: KeyValueRef) {
        if let Some(value) = kvr.value {
            self.put(kvr.key, kvr.timestamp, value);
        } else {
            self.del(kvr.key, kvr.timestamp);
        }
    }

    /// Put the key@timestamp => value.
    pub fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        self.setsum
            .insert_vectored(&[&[8], key, &timestamp.to_le_bytes(), value]);
    }

    /// Put the key@timestamp => TOMBSTONE.
    pub fn del(&mut self, key: &[u8], timestamp: u64) {
        self.setsum
            .insert_vectored(&[&[9], key, &timestamp.to_le_bytes()]);
    }

    /// Get the underlying setsum.
    pub fn into_inner(self) -> RawSetsum {
        self.setsum
    }
}

impl std::ops::Add<Setsum> for Setsum {
    type Output = Setsum;

    fn add(self, rhs: Setsum) -> Setsum {
        Setsum {
            setsum: self.setsum + rhs.setsum,
        }
    }
}

impl std::ops::AddAssign<Setsum> for Setsum {
    fn add_assign(&mut self, rhs: Setsum) {
        self.setsum += rhs.setsum;
    }
}

impl std::ops::Sub<Setsum> for Setsum {
    type Output = Setsum;

    fn sub(self, rhs: Setsum) -> Setsum {
        Setsum {
            setsum: self.setsum - rhs.setsum,
        }
    }
}

impl std::ops::SubAssign<Setsum> for Setsum {
    fn sub_assign(&mut self, rhs: Setsum) {
        self.setsum -= rhs.setsum;
    }
}
