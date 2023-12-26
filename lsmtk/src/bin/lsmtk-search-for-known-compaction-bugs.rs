use std::cmp::Ordering;
use std::collections::HashSet;
use std::ops::Add;

use arrrg::CommandLine;
use keyvalint::compare_bytes;
use setsum::Setsum;

use lsmtk::{CompactionID, TreeLogKey, TreeLogValue};

#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct DebuggerOptions {
    #[arrrg(optional, "Search for this compaction.")]
    compaction: Option<String>,
    #[arrrg(required, "Search through this debug log.")]
    debug_log: String,
}

fn debug_one(compaction_id: CompactionID, debug_log: String) {
    let filter_key = TreeLogKey::ByCompactionID { compaction_id };
    let protobuf = indicio::read_protobuf_file::<_, TreeLogKey, TreeLogValue>(&debug_log)
        .expect("debug log should parse");
    let mut compaction = None;
    let mut apply = None;
    let mut compact_level = vec![];
    let mut compact_upper_level = None;
    for record in protobuf.records.into_iter() {
        if record.key == filter_key {
            if let TreeLogValue::CandidateCompaction { .. } = &record.value {
                compaction = Some(record.value);
            } else if let TreeLogValue::ApplyCompaction { .. } = &record.value {
                if apply.is_some() {
                    println!("{compaction_id}: apply has two candidate records; this should never happen and has never been observed");
                    println!(
                        "{compaction_id}: first record: {:#?}",
                        apply.as_ref().unwrap()
                    );
                    println!("{compaction_id}: second record: {:#?}", record.value);
                } else {
                    apply = Some(record.value);
                }
            } else if let TreeLogValue::CompactLevel {
                level,
                before,
                after,
            } = record.value
            {
                compact_level.push((level, before.clone(), after.clone()));
            } else if let TreeLogValue::CompactUpperLevelBounds {
                level,
                lower_bound,
                upper_bound,
            } = record.value
            {
                compact_upper_level = Some((level, lower_bound, upper_bound));
            }
        }
    }
    // Sanity check the compaction record.
    let (first_key, last_key, lower_level, upper_level, inputs) = match compaction {
        Some(TreeLogValue::CandidateCompaction {
            first_key,
            last_key,
            lower_level,
            upper_level,
            inputs,
            ..
        }) => (first_key, last_key, lower_level, upper_level, inputs),
        Some(_) => {
            panic!("{compaction_id}: this should be impossible");
        }
        None => {
            // NOTE(rescrv): This is perfectly acceptable if there is no candidate compaction.
            if apply.is_some() {
                println!("{compaction_id}: found no compaction record to match with apply record");
            }
            return;
        }
    };
    if compare_bytes(&first_key, &last_key) == Ordering::Greater {
        println!("{compaction_id}: compaction has start key greater than end key; this should never happen and has never been observed");
    }
    if lower_level == upper_level {
        println!("{compaction_id}: compaction has lower_level = upper_level = {lower_level}");
    }
    let inputs_setsum = inputs
        .iter()
        .copied()
        .map(Setsum::from_digest)
        .fold(Setsum::default(), Setsum::add);
    // Sanity check the apply record.
    let outputs = match apply {
        Some(TreeLogValue::ApplyCompaction { outputs, .. }) => outputs,
        Some(_) => {
            panic!("{compaction_id}: this should be impossible");
        }
        None => {
            println!("{compaction_id}: found no apply record");
            return;
        }
    };
    let outputs_setsum = outputs
        .iter()
        .copied()
        .map(Setsum::from_digest)
        .fold(Setsum::default(), Setsum::add);
    // Cross-check the inputs and outputs.
    if inputs_setsum != outputs_setsum {
        println!(
            "{compaction_id}: setsum mismatch: inputs={:?} != outputs={:?}",
            inputs_setsum, outputs_setsum
        );
    }
    // Check the CompactLevel case.
    let mut last_level = 0;
    let mut last_before = vec![];
    let mut last_after = vec![];
    for (level, before, after) in compact_level.into_iter() {
        let removed = before
            .iter()
            .filter(|x| !after.contains(x))
            .copied()
            .collect::<Vec<_>>();
        let added = after
            .iter()
            .filter(|x| !before.contains(x))
            .copied()
            .collect::<Vec<_>>();
        for rm in removed.iter() {
            if !inputs.contains(rm) {
                println!(
                    "{compaction_id}: inadvertently removed {:?} from level {}",
                    Setsum::from_digest(*rm),
                    level
                );
            }
        }
        for add in added.iter() {
            if !outputs.contains(add) {
                println!(
                    "{compaction_id}: inadvertently added {:?} to level {}",
                    Setsum::from_digest(*add),
                    level
                );
            }
        }
        last_level = level;
        last_before = before;
        last_after = after;
    }
    // Check for CompactUpperLevelBounds
    if let Some((level, lower_bound, upper_bound)) = compact_upper_level {
        if last_level != level {
            println!(
                "{compaction_id}: CompactUpperLevelBounds does not match last CompactLevel record"
            );
        } else if last_before.len() - (upper_bound - lower_bound) + outputs.len()
            != last_after.len()
        {
            println!("{compaction_id}: before and after do not balance.");
        }
    } else {
        println!("{compaction_id}: did not emit CompactUpperLevelBounds record");
    }
}

fn main() {
    let (options, files) = DebuggerOptions::from_command_line_relaxed(
        "USAGE: lsmtk-search-for-known-compaction-bugs [OPTIONS]",
    );
    if !files.is_empty() {
        println!("expected no positional arguments");
        std::process::exit(1);
    }
    if let Some(compaction) = options.compaction.as_ref() {
        let compaction_id: CompactionID =
            CompactionID::from_human_readable(compaction).expect("--compaction should parse");
        debug_one(compaction_id, options.debug_log);
    } else {
        let protobuf =
            indicio::read_protobuf_file::<_, TreeLogKey, TreeLogValue>(&options.debug_log)
                .expect("debug log should parse");
        let mut compactions: HashSet<CompactionID> = HashSet::default();
        for record in protobuf.records.into_iter() {
            if let TreeLogKey::ByCompactionID { compaction_id } = record.key {
                compactions.insert(compaction_id);
            }
        }
        let mut compactions: Vec<CompactionID> = compactions.into_iter().collect();
        compactions.sort();
        for compaction_id in compactions.into_iter() {
            debug_one(compaction_id, options.debug_log.clone());
        }
    }
    println!(
        "If you're reading this as the first line of output, there are no known bugs observed."
    );
}
