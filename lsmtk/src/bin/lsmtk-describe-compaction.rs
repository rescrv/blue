use arrrg::CommandLine;

use lsmtk::{TreeLogKey, TreeLogValue, NUM_LEVELS};

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct DebuggerOptions {
    #[arrrg(required, "Search through this debug log.")]
    debug_log: String,
}

fn main() {
    let (options, files) = DebuggerOptions::from_command_line_relaxed(
        "USAGE: lsmtk-search-for-known-compaction-bugs [OPTIONS]",
    );
    if !files.is_empty() {
        println!("expected no positional arguments");
        std::process::exit(1);
    }
    let protobuf = indicio::read_protobuf_file::<_, TreeLogKey, TreeLogValue>(&options.debug_log)
        .expect("debug log should parse");
    let mut lowest = None;
    let mut levels = vec![0; NUM_LEVELS];
    let mut prev_levels: Option<Vec<usize>> = None;
    for record in protobuf.records.into_iter() {
        if let TreeLogValue::Ingest { level, .. } = record.value {
            levels[level] += 1;
        } else if let TreeLogValue::ApplyCompaction { outputs: _ } = record.value {
            lowest = None;
        } else if let TreeLogValue::CompactLevel {
            level,
            before,
            after,
        } = record.value
        {
            if lowest.is_none() {
                lowest = Some(level);
            }
            if before.len() > after.len() {
                levels[level] -= before.len() - after.len();
            } else {
                levels[level] += after.len() - before.len();
            }
        } else if let TreeLogValue::CompactUpperLevelBounds {
            level,
            lower_bound: _,
            upper_bound: _,
        } = record.value
        {
            if let Some(levels) = prev_levels {
                let lower = lowest.expect("compaction should always have at least one level");
                let upper = level;
                let mut first_line = "[".to_string();
                let mut second_line = " ".to_string();
                for (idx, level) in levels.iter().enumerate().take(NUM_LEVELS) {
                    if idx > 0 {
                        first_line += ", ";
                    }
                    if idx == lower {
                        while first_line.chars().count() > second_line.chars().count() {
                            second_line.push(' ');
                        }
                        second_line.push('╰');
                    }
                    first_line += &format!("{}", level);
                    if idx == upper {
                        while first_line.chars().count() > second_line.chars().count() {
                            second_line.push('─');
                        }
                        second_line.push('╯');
                    }
                }
                first_line.push(']');
                if upper >= NUM_LEVELS {
                    while first_line.len() > second_line.len() + 1 {
                        second_line.push('─');
                    }
                    second_line.push('╯');
                }
                println!("{}\n{}", first_line, second_line);
            }
            prev_levels = Some(levels.clone());
        }
    }
}
