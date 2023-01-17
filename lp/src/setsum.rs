use std::fmt::Write;

use setsum::Setsum as RawSetsum;

////////////////////////////////////////////// Setsum //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct Setsum {
    setsum: RawSetsum,
}

impl Setsum {
    pub fn digest(&self) -> [u8; 32] {
        self.setsum.digest()
    }

    pub fn hexdigest(&self) -> String {
        let digest = self.setsum.digest();
        let mut setsum = String::with_capacity(68);
        for i in 0..digest.len() {
            write!(&mut setsum, "{:02x}", digest[i]).expect("unable to write to string");
        }
        setsum
    }

    pub fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        self.setsum.insert_vectored(&[&[8], key, &timestamp.to_le_bytes(), value]);
    }

    pub fn del(&mut self, key: &[u8], timestamp: u64) {
        self.setsum.insert_vectored(&[&[9], key, &timestamp.to_le_bytes()]);
    }
}
