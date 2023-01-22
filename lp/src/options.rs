use super::sst::SSTBuilderOptions;
use super::TABLE_FULL_SIZE;

///////////////////////////////////////// CompactionOptions ////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct CompactionOptions {
    pub max_compaction_bytes: u64,
    pub sst_options: SSTBuilderOptions,
}

impl CompactionOptions {
    pub fn clamp(&mut self) {
        if self.max_compaction_bytes < TABLE_FULL_SIZE as u64 {
            self.max_compaction_bytes = TABLE_FULL_SIZE as u64;
        }
    }
}

impl Default for CompactionOptions {
    fn default() -> Self {
        Self {
            max_compaction_bytes: TABLE_FULL_SIZE as u64 * 128,
            sst_options: SSTBuilderOptions::default(),
        }
    }
}
