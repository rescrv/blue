extern crate lp;

use lp::pruning_cursor::PruningCursor;
use lp::reference::{ReferenceBuilder, ReferenceCursor};

mod alphabet;

fn pruning_cursor_no_pruning(_: &str) -> PruningCursor<ReferenceCursor> {
    let mut builder = ReferenceBuilder::default();
    builder.put("A".as_bytes(), 0, "a".as_bytes()).unwrap();
    builder.put("B".as_bytes(), 0, "b".as_bytes()).unwrap();
    builder.put("C".as_bytes(), 0, "c".as_bytes()).unwrap();
    builder.put("D".as_bytes(), 0, "d".as_bytes()).unwrap();
    builder.put("E".as_bytes(), 0, "e".as_bytes()).unwrap();
    builder.put("F".as_bytes(), 0, "f".as_bytes()).unwrap();
    builder.put("G".as_bytes(), 0, "g".as_bytes()).unwrap();
    builder.put("H".as_bytes(), 0, "h".as_bytes()).unwrap();
    builder.put("I".as_bytes(), 0, "i".as_bytes()).unwrap();
    builder.put("J".as_bytes(), 0, "j".as_bytes()).unwrap();
    builder.put("K".as_bytes(), 0, "k".as_bytes()).unwrap();
    builder.put("L".as_bytes(), 0, "l".as_bytes()).unwrap();
    builder.put("M".as_bytes(), 0, "m".as_bytes()).unwrap();
    builder.put("N".as_bytes(), 0, "n".as_bytes()).unwrap();
    builder.put("O".as_bytes(), 0, "o".as_bytes()).unwrap();
    builder.put("P".as_bytes(), 0, "p".as_bytes()).unwrap();
    builder.put("Q".as_bytes(), 0, "q".as_bytes()).unwrap();
    builder.put("R".as_bytes(), 0, "r".as_bytes()).unwrap();
    builder.put("S".as_bytes(), 0, "s".as_bytes()).unwrap();
    builder.put("T".as_bytes(), 0, "t".as_bytes()).unwrap();
    builder.put("U".as_bytes(), 0, "u".as_bytes()).unwrap();
    builder.put("V".as_bytes(), 0, "v".as_bytes()).unwrap();
    builder.put("W".as_bytes(), 0, "w".as_bytes()).unwrap();
    builder.put("X".as_bytes(), 0, "x".as_bytes()).unwrap();
    builder.put("Y".as_bytes(), 0, "y".as_bytes()).unwrap();
    builder.put("Z".as_bytes(), 0, "z".as_bytes()).unwrap();
    PruningCursor::new(builder.seal().unwrap().cursor(), 10).unwrap()
}

alphabet_tests! {
    pruning_cursor_no_pruning: crate::pruning_cursor_no_pruning,
}

fn pruning_cursor_deleted_extras(_: &str) -> PruningCursor<ReferenceCursor> {
    let mut builder = ReferenceBuilder::default();
    builder.put("A".as_bytes(), 0, "a".as_bytes()).unwrap();
    builder.del("AA".as_bytes(), 1).unwrap();
    builder.put("AA".as_bytes(), 0, "aa".as_bytes()).unwrap();
    builder.put("B".as_bytes(), 0, "b".as_bytes()).unwrap();
    builder.del("BB".as_bytes(), 1).unwrap();
    builder.put("BB".as_bytes(), 0, "bb".as_bytes()).unwrap();
    builder.put("C".as_bytes(), 0, "c".as_bytes()).unwrap();
    builder.del("CC".as_bytes(), 1).unwrap();
    builder.put("CC".as_bytes(), 0, "cc".as_bytes()).unwrap();
    builder.put("D".as_bytes(), 0, "d".as_bytes()).unwrap();
    builder.del("DD".as_bytes(), 1).unwrap();
    builder.put("DD".as_bytes(), 0, "dd".as_bytes()).unwrap();
    builder.put("E".as_bytes(), 0, "e".as_bytes()).unwrap();
    builder.del("EE".as_bytes(), 1).unwrap();
    builder.put("EE".as_bytes(), 0, "ee".as_bytes()).unwrap();
    builder.put("F".as_bytes(), 0, "f".as_bytes()).unwrap();
    builder.del("FF".as_bytes(), 1).unwrap();
    builder.put("FF".as_bytes(), 0, "ff".as_bytes()).unwrap();
    builder.put("G".as_bytes(), 0, "g".as_bytes()).unwrap();
    builder.del("GG".as_bytes(), 1).unwrap();
    builder.put("GG".as_bytes(), 0, "gg".as_bytes()).unwrap();
    builder.put("H".as_bytes(), 0, "h".as_bytes()).unwrap();
    builder.del("HH".as_bytes(), 1).unwrap();
    builder.put("HH".as_bytes(), 0, "hh".as_bytes()).unwrap();
    builder.put("I".as_bytes(), 0, "i".as_bytes()).unwrap();
    builder.del("II".as_bytes(), 1).unwrap();
    builder.put("II".as_bytes(), 0, "ii".as_bytes()).unwrap();
    builder.put("J".as_bytes(), 0, "j".as_bytes()).unwrap();
    builder.del("JJ".as_bytes(), 1).unwrap();
    builder.put("JJ".as_bytes(), 0, "jj".as_bytes()).unwrap();
    builder.put("K".as_bytes(), 0, "k".as_bytes()).unwrap();
    builder.del("KK".as_bytes(), 1).unwrap();
    builder.put("KK".as_bytes(), 0, "kk".as_bytes()).unwrap();
    builder.put("L".as_bytes(), 0, "l".as_bytes()).unwrap();
    builder.del("LL".as_bytes(), 1).unwrap();
    builder.put("LL".as_bytes(), 0, "ll".as_bytes()).unwrap();
    builder.put("M".as_bytes(), 0, "m".as_bytes()).unwrap();
    builder.del("MM".as_bytes(), 1).unwrap();
    builder.put("MM".as_bytes(), 0, "mm".as_bytes()).unwrap();
    builder.put("N".as_bytes(), 0, "n".as_bytes()).unwrap();
    builder.del("NN".as_bytes(), 1).unwrap();
    builder.put("NN".as_bytes(), 0, "nn".as_bytes()).unwrap();
    builder.put("O".as_bytes(), 0, "o".as_bytes()).unwrap();
    builder.del("OO".as_bytes(), 1).unwrap();
    builder.put("OO".as_bytes(), 0, "oo".as_bytes()).unwrap();
    builder.put("P".as_bytes(), 0, "p".as_bytes()).unwrap();
    builder.del("PP".as_bytes(), 1).unwrap();
    builder.put("PP".as_bytes(), 0, "pp".as_bytes()).unwrap();
    builder.put("Q".as_bytes(), 0, "q".as_bytes()).unwrap();
    builder.del("QQ".as_bytes(), 1).unwrap();
    builder.put("QQ".as_bytes(), 0, "qq".as_bytes()).unwrap();
    builder.put("R".as_bytes(), 0, "r".as_bytes()).unwrap();
    builder.del("RR".as_bytes(), 1).unwrap();
    builder.put("RR".as_bytes(), 0, "rr".as_bytes()).unwrap();
    builder.put("S".as_bytes(), 0, "s".as_bytes()).unwrap();
    builder.del("SS".as_bytes(), 1).unwrap();
    builder.put("SS".as_bytes(), 0, "ss".as_bytes()).unwrap();
    builder.put("T".as_bytes(), 0, "t".as_bytes()).unwrap();
    builder.del("TT".as_bytes(), 1).unwrap();
    builder.put("TT".as_bytes(), 0, "tt".as_bytes()).unwrap();
    builder.put("U".as_bytes(), 0, "u".as_bytes()).unwrap();
    builder.del("UU".as_bytes(), 1).unwrap();
    builder.put("UU".as_bytes(), 0, "uu".as_bytes()).unwrap();
    builder.put("V".as_bytes(), 0, "v".as_bytes()).unwrap();
    builder.del("VV".as_bytes(), 1).unwrap();
    builder.put("VV".as_bytes(), 0, "vv".as_bytes()).unwrap();
    builder.put("W".as_bytes(), 0, "w".as_bytes()).unwrap();
    builder.del("WW".as_bytes(), 1).unwrap();
    builder.put("WW".as_bytes(), 0, "ww".as_bytes()).unwrap();
    builder.put("X".as_bytes(), 0, "x".as_bytes()).unwrap();
    builder.del("XX".as_bytes(), 1).unwrap();
    builder.put("XX".as_bytes(), 0, "xx".as_bytes()).unwrap();
    builder.put("Y".as_bytes(), 0, "y".as_bytes()).unwrap();
    builder.del("YY".as_bytes(), 1).unwrap();
    builder.put("YY".as_bytes(), 0, "yy".as_bytes()).unwrap();
    builder.put("Z".as_bytes(), 0, "z".as_bytes()).unwrap();
    builder.del("ZZ".as_bytes(), 1).unwrap();
    builder.put("ZZ".as_bytes(), 0, "zz".as_bytes()).unwrap();
    PruningCursor::new(builder.seal().unwrap().cursor(), 10).unwrap()
}

alphabet_tests! {
    pruning_cursor_deleted_extras: crate::pruning_cursor_deleted_extras,
}

fn pruning_cursor_snapshot_cutoff(_: &str) -> PruningCursor<ReferenceCursor> {
    let mut builder = ReferenceBuilder::default();
    builder.put("A".as_bytes(), 7, "a7".as_bytes()).unwrap();
    builder.put("A".as_bytes(), 0, "a".as_bytes()).unwrap();
    builder.put("B".as_bytes(), 7, "b7".as_bytes()).unwrap();
    builder.put("B".as_bytes(), 0, "b".as_bytes()).unwrap();
    builder.put("C".as_bytes(), 7, "c7".as_bytes()).unwrap();
    builder.put("C".as_bytes(), 0, "c".as_bytes()).unwrap();
    builder.put("D".as_bytes(), 7, "d7".as_bytes()).unwrap();
    builder.put("D".as_bytes(), 0, "d".as_bytes()).unwrap();
    builder.put("E".as_bytes(), 7, "e7".as_bytes()).unwrap();
    builder.put("E".as_bytes(), 0, "e".as_bytes()).unwrap();
    builder.put("F".as_bytes(), 7, "f7".as_bytes()).unwrap();
    builder.put("F".as_bytes(), 0, "f".as_bytes()).unwrap();
    builder.put("G".as_bytes(), 7, "g7".as_bytes()).unwrap();
    builder.put("G".as_bytes(), 0, "g".as_bytes()).unwrap();
    builder.put("H".as_bytes(), 7, "h7".as_bytes()).unwrap();
    builder.put("H".as_bytes(), 0, "h".as_bytes()).unwrap();
    builder.put("I".as_bytes(), 7, "i7".as_bytes()).unwrap();
    builder.put("I".as_bytes(), 0, "i".as_bytes()).unwrap();
    builder.put("J".as_bytes(), 7, "j7".as_bytes()).unwrap();
    builder.put("J".as_bytes(), 0, "j".as_bytes()).unwrap();
    builder.put("K".as_bytes(), 7, "k7".as_bytes()).unwrap();
    builder.put("K".as_bytes(), 0, "k".as_bytes()).unwrap();
    builder.put("L".as_bytes(), 7, "l7".as_bytes()).unwrap();
    builder.put("L".as_bytes(), 0, "l".as_bytes()).unwrap();
    builder.put("M".as_bytes(), 7, "m7".as_bytes()).unwrap();
    builder.put("M".as_bytes(), 0, "m".as_bytes()).unwrap();
    builder.put("N".as_bytes(), 7, "n7".as_bytes()).unwrap();
    builder.put("N".as_bytes(), 0, "n".as_bytes()).unwrap();
    builder.put("O".as_bytes(), 7, "o7".as_bytes()).unwrap();
    builder.put("O".as_bytes(), 0, "o".as_bytes()).unwrap();
    builder.put("P".as_bytes(), 7, "p7".as_bytes()).unwrap();
    builder.put("P".as_bytes(), 0, "p".as_bytes()).unwrap();
    builder.put("Q".as_bytes(), 7, "q7".as_bytes()).unwrap();
    builder.put("Q".as_bytes(), 0, "q".as_bytes()).unwrap();
    builder.put("R".as_bytes(), 7, "r7".as_bytes()).unwrap();
    builder.put("R".as_bytes(), 0, "r".as_bytes()).unwrap();
    builder.put("S".as_bytes(), 7, "s7".as_bytes()).unwrap();
    builder.put("S".as_bytes(), 0, "s".as_bytes()).unwrap();
    builder.put("T".as_bytes(), 7, "t7".as_bytes()).unwrap();
    builder.put("T".as_bytes(), 0, "t".as_bytes()).unwrap();
    builder.put("U".as_bytes(), 7, "u7".as_bytes()).unwrap();
    builder.put("U".as_bytes(), 0, "u".as_bytes()).unwrap();
    builder.put("V".as_bytes(), 7, "v7".as_bytes()).unwrap();
    builder.put("V".as_bytes(), 0, "v".as_bytes()).unwrap();
    builder.put("W".as_bytes(), 7, "w7".as_bytes()).unwrap();
    builder.put("W".as_bytes(), 0, "w".as_bytes()).unwrap();
    builder.put("X".as_bytes(), 7, "x7".as_bytes()).unwrap();
    builder.put("X".as_bytes(), 0, "x".as_bytes()).unwrap();
    builder.put("Y".as_bytes(), 7, "y7".as_bytes()).unwrap();
    builder.put("Y".as_bytes(), 0, "y".as_bytes()).unwrap();
    builder.put("Z".as_bytes(), 7, "z7".as_bytes()).unwrap();
    builder.put("Z".as_bytes(), 0, "z".as_bytes()).unwrap();
    PruningCursor::new(builder.seal().unwrap().cursor(), 5).unwrap()
}

alphabet_tests! {
    pruning_cursor_snapshot_cutoff: crate::pruning_cursor_snapshot_cutoff,
}

fn pruning_cursor_tombstone_above_snapshot(_: &str) -> PruningCursor<ReferenceCursor> {
    let mut builder = ReferenceBuilder::default();
    builder.del("A".as_bytes(), 7).unwrap();
    builder.put("A".as_bytes(), 0, "a".as_bytes()).unwrap();
    builder.del("B".as_bytes(), 7).unwrap();
    builder.put("B".as_bytes(), 0, "b".as_bytes()).unwrap();
    builder.del("C".as_bytes(), 7).unwrap();
    builder.put("C".as_bytes(), 0, "c".as_bytes()).unwrap();
    builder.del("D".as_bytes(), 7).unwrap();
    builder.put("D".as_bytes(), 0, "d".as_bytes()).unwrap();
    builder.del("E".as_bytes(), 7).unwrap();
    builder.put("E".as_bytes(), 0, "e".as_bytes()).unwrap();
    builder.del("F".as_bytes(), 7).unwrap();
    builder.put("F".as_bytes(), 0, "f".as_bytes()).unwrap();
    builder.del("G".as_bytes(), 7).unwrap();
    builder.put("G".as_bytes(), 0, "g".as_bytes()).unwrap();
    builder.del("H".as_bytes(), 7).unwrap();
    builder.put("H".as_bytes(), 0, "h".as_bytes()).unwrap();
    builder.del("I".as_bytes(), 7).unwrap();
    builder.put("I".as_bytes(), 0, "i".as_bytes()).unwrap();
    builder.del("J".as_bytes(), 7).unwrap();
    builder.put("J".as_bytes(), 0, "j".as_bytes()).unwrap();
    builder.del("K".as_bytes(), 7).unwrap();
    builder.put("K".as_bytes(), 0, "k".as_bytes()).unwrap();
    builder.del("L".as_bytes(), 7).unwrap();
    builder.put("L".as_bytes(), 0, "l".as_bytes()).unwrap();
    builder.del("M".as_bytes(), 7).unwrap();
    builder.put("M".as_bytes(), 0, "m".as_bytes()).unwrap();
    builder.del("N".as_bytes(), 7).unwrap();
    builder.put("N".as_bytes(), 0, "n".as_bytes()).unwrap();
    builder.del("O".as_bytes(), 7).unwrap();
    builder.put("O".as_bytes(), 0, "o".as_bytes()).unwrap();
    builder.del("P".as_bytes(), 7).unwrap();
    builder.put("P".as_bytes(), 0, "p".as_bytes()).unwrap();
    builder.del("Q".as_bytes(), 7).unwrap();
    builder.put("Q".as_bytes(), 0, "q".as_bytes()).unwrap();
    builder.del("R".as_bytes(), 7).unwrap();
    builder.put("R".as_bytes(), 0, "r".as_bytes()).unwrap();
    builder.del("S".as_bytes(), 7).unwrap();
    builder.put("S".as_bytes(), 0, "s".as_bytes()).unwrap();
    builder.del("T".as_bytes(), 7).unwrap();
    builder.put("T".as_bytes(), 0, "t".as_bytes()).unwrap();
    builder.del("U".as_bytes(), 7).unwrap();
    builder.put("U".as_bytes(), 0, "u".as_bytes()).unwrap();
    builder.del("V".as_bytes(), 7).unwrap();
    builder.put("V".as_bytes(), 0, "v".as_bytes()).unwrap();
    builder.del("W".as_bytes(), 7).unwrap();
    builder.put("W".as_bytes(), 0, "w".as_bytes()).unwrap();
    builder.del("X".as_bytes(), 7).unwrap();
    builder.put("X".as_bytes(), 0, "x".as_bytes()).unwrap();
    builder.del("Y".as_bytes(), 7).unwrap();
    builder.put("Y".as_bytes(), 0, "y".as_bytes()).unwrap();
    builder.del("Z".as_bytes(), 7).unwrap();
    builder.put("Z".as_bytes(), 0, "z".as_bytes()).unwrap();
    PruningCursor::new(builder.seal().unwrap().cursor(), 5).unwrap()
}

alphabet_tests! {
    pruning_cursor_tombstone_above_snapshot: crate::pruning_cursor_tombstone_above_snapshot,
}
