extern crate sst;

use sst::reference::{ReferenceBuilder, ReferenceCursor, ReferenceTable};
use sst::concat_cursor::ConcatenatingCursor;

mod alphabet;

fn sequence_cursor(_: &str) -> ConcatenatingCursor<ReferenceCursor> {
    let mut tables: Vec<ReferenceTable> = Vec::new();
    let mut builder = ReferenceBuilder::default();
    builder.put("A".as_bytes(), 0, "a".as_bytes()).unwrap();
    builder.put("B".as_bytes(), 0, "b".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("C".as_bytes(), 0, "c".as_bytes()).unwrap();
    builder.put("D".as_bytes(), 0, "d".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("E".as_bytes(), 0, "e".as_bytes()).unwrap();
    builder.put("F".as_bytes(), 0, "f".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("G".as_bytes(), 0, "g".as_bytes()).unwrap();
    builder.put("H".as_bytes(), 0, "h".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("I".as_bytes(), 0, "i".as_bytes()).unwrap();
    builder.put("J".as_bytes(), 0, "j".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("K".as_bytes(), 0, "k".as_bytes()).unwrap();
    builder.put("L".as_bytes(), 0, "l".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("M".as_bytes(), 0, "m".as_bytes()).unwrap();
    builder.put("N".as_bytes(), 0, "n".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("O".as_bytes(), 0, "o".as_bytes()).unwrap();
    builder.put("P".as_bytes(), 0, "p".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("Q".as_bytes(), 0, "q".as_bytes()).unwrap();
    builder.put("R".as_bytes(), 0, "r".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("S".as_bytes(), 0, "s".as_bytes()).unwrap();
    builder.put("T".as_bytes(), 0, "t".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("U".as_bytes(), 0, "u".as_bytes()).unwrap();
    builder.put("V".as_bytes(), 0, "v".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("W".as_bytes(), 0, "w".as_bytes()).unwrap();
    builder.put("X".as_bytes(), 0, "x".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let mut builder = ReferenceBuilder::default();
    builder.put("Y".as_bytes(), 0, "y".as_bytes()).unwrap();
    builder.put("Z".as_bytes(), 0, "z".as_bytes()).unwrap();
    tables.push(builder.seal().unwrap());
    let cursors = tables.into_iter().map(|t| t.cursor()).collect();
    ConcatenatingCursor::new(cursors).unwrap()
}

alphabet_tests! {
    sequence_cursor: crate::sequence_cursor,
}
