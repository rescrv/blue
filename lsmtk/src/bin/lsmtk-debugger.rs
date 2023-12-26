use arrrg::CommandLine;
use indicio::Debugger;
use setsum::Setsum;

use lsmtk::{CompactionID, TreeLogKey, TreeLogValue};

struct KeyDisplay {}

impl indicio::Display<TreeLogKey> for KeyDisplay {
    fn display(&mut self, key: &TreeLogKey) -> String {
        format!("{:?}", key)
    }
}

struct ValueDisplay {}

impl indicio::Display<TreeLogValue> for ValueDisplay {
    fn display(&mut self, value: &TreeLogValue) -> String {
        format!("{:?}", value)
    }
}

struct PrettyValueDisplay {}

impl indicio::Display<TreeLogValue> for PrettyValueDisplay {
    fn display(&mut self, value: &TreeLogValue) -> String {
        format!("{:#?}", value)
    }
}

struct SetsumFilter {
    setsum: [u8; 32],
}

impl indicio::Filter<TreeLogKey, TreeLogValue> for SetsumFilter {
    fn matches(&mut self, key: &TreeLogKey, value: &TreeLogValue) -> bool {
        let matches_key = match key {
            TreeLogKey::Nop => false,
            TreeLogKey::BySetsum { setsum } => *setsum == self.setsum,
            TreeLogKey::ByCompactionID { compaction_id: _ } => false,
        };
        let matches_value = match value {
            TreeLogValue::Nop => false,
            TreeLogValue::Ingest {
                level: _,
                cardinality: _,
            } => false,
            TreeLogValue::CandidateCompaction {
                score: _,
                lower_level: _,
                upper_level: _,
                first_key: _,
                last_key: _,
                inputs,
            } => inputs.contains(&self.setsum),
            TreeLogValue::GatherInput {} => false,
            TreeLogValue::RemoveCompactionDir { dir: _ } => false,
            TreeLogValue::ApplyCompaction { outputs } => outputs.contains(&self.setsum),
            TreeLogValue::CompactLevel {
                level: _,
                before,
                after,
            } => before.contains(&self.setsum) || after.contains(&self.setsum),
            TreeLogValue::CompactUpperLevelBounds {
                level: _,
                lower_bound: _,
                upper_bound: _,
            } => false,
        };
        matches_key || matches_value
    }
}

struct CompactionFilter {
    compaction_id: CompactionID,
}

impl indicio::Filter<TreeLogKey, TreeLogValue> for CompactionFilter {
    fn matches(&mut self, key: &TreeLogKey, _: &TreeLogValue) -> bool {
        match key {
            TreeLogKey::Nop => false,
            TreeLogKey::BySetsum { setsum: _ } => false,
            TreeLogKey::ByCompactionID { compaction_id } => *compaction_id == self.compaction_id,
        }
    }
}

struct LevelFilter {
    level: usize,
}

impl indicio::Filter<TreeLogKey, TreeLogValue> for LevelFilter {
    fn matches(&mut self, _: &TreeLogKey, value: &TreeLogValue) -> bool {
        match value {
            TreeLogValue::Nop => false,
            TreeLogValue::Ingest {
                level,
                cardinality: _,
            } => self.level == *level,
            TreeLogValue::CandidateCompaction {
                score: _,
                lower_level,
                upper_level,
                first_key: _,
                last_key: _,
                inputs: _,
            } => (*lower_level..=*upper_level).contains(&self.level),
            TreeLogValue::GatherInput {} => false,
            TreeLogValue::RemoveCompactionDir { dir: _ } => false,
            TreeLogValue::ApplyCompaction { outputs: _ } => false,
            TreeLogValue::CompactLevel {
                level,
                before: _,
                after: _,
            } => self.level == *level,
            TreeLogValue::CompactUpperLevelBounds {
                level,
                lower_bound: _,
                upper_bound: _,
            } => self.level == *level,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct DebuggerOptions {
    #[arrrg(optional, "Output every record that contains this setsum.")]
    setsum: Option<String>,
    #[arrrg(optional, "Output every record that contains this compaction.")]
    compaction: Option<String>,
    #[arrrg(optional, "Output every record that contains this level.")]
    level: Option<usize>,
    #[arrrg(flag, "Output using something akin to :#? instead of the default.")]
    pretty_print: bool,
}

fn main() {
    let (options, files) =
        DebuggerOptions::from_command_line_relaxed("USAGE: lsmtk-debugger [OPTIONS] <file ...>");
    let mut debugger = Debugger::<TreeLogKey, TreeLogValue>::default();
    debugger.add_key_display(KeyDisplay {});
    if options.pretty_print {
        debugger.add_value_display(PrettyValueDisplay {});
    } else {
        debugger.add_value_display(ValueDisplay {});
    }
    if let Some(hexdigest) = options.setsum {
        let setsum = Setsum::from_hexdigest(&hexdigest)
            .expect("--setsum should parse")
            .digest();
        debugger.add_filter(SetsumFilter { setsum });
    }
    if let Some(compaction) = options.compaction {
        let compaction_id: CompactionID =
            CompactionID::from_human_readable(&compaction).expect("--compaction should parse");
        debugger.add_filter(CompactionFilter { compaction_id });
    }
    if let Some(level) = options.level {
        debugger.add_filter(LevelFilter { level });
    }
    for file in files.into_iter() {
        debugger
            .execute(file, &mut std::io::stdout())
            .expect("debugger should not fail");
    }
}
