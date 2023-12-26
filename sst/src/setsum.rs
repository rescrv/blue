use keyvalint::KeyValueRef;
use setsum::Setsum as RawSetsum;

pub use setsum::SETSUM_BYTES;

////////////////////////////////////////////// Setsum //////////////////////////////////////////////

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Setsum {
    setsum: RawSetsum,
}

impl Setsum {
    pub fn digest(&self) -> [u8; 32] {
        self.setsum.digest()
    }

    pub fn from_digest(digest: [u8; SETSUM_BYTES]) -> Setsum {
        let setsum = RawSetsum::from_digest(digest);
        Self { setsum }
    }

    pub fn hexdigest(&self) -> String {
        self.setsum.hexdigest()
    }

    pub fn from_hexdigest(digest: &str) -> Option<Setsum> {
        let setsum = RawSetsum::from_hexdigest(digest)?;
        Some(Setsum { setsum })
    }

    pub fn insert(&mut self, kvr: KeyValueRef) {
        if let Some(value) = kvr.value {
            self.put(kvr.key, kvr.timestamp, value);
        } else {
            self.del(kvr.key, kvr.timestamp);
        }
    }

    pub fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        self.setsum
            .insert_vectored(&[&[8], key, &timestamp.to_le_bytes(), value]);
    }

    pub fn del(&mut self, key: &[u8], timestamp: u64) {
        self.setsum
            .insert_vectored(&[&[9], key, &timestamp.to_le_bytes()]);
    }

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
