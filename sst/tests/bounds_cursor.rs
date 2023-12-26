extern crate sst;

use std::ops::Bound;

use sst::bounds_cursor::BoundsCursor;
use sst::reference::{ReferenceBuilder, ReferenceCursor};
use sst::Error;

mod alphabet;

fn bounds_cursor_no_bounds(_: &str) -> BoundsCursor<ReferenceCursor, Error> {
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
    BoundsCursor::new(
        builder.seal().unwrap().cursor(),
        &Bound::<&[u8]>::Unbounded,
        &Bound::<&[u8]>::Unbounded,
    )
    .unwrap()
}

alphabet_tests! {
    bounds_cursor_no_bounds: crate::bounds_cursor_no_bounds,
}

fn bounds_cursor_start_bound_included(_: &str) -> BoundsCursor<ReferenceCursor, Error> {
    let mut builder = ReferenceBuilder::default();
    builder.put("@".as_bytes(), 0, "@".as_bytes()).unwrap();
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
    BoundsCursor::new(
        builder.seal().unwrap().cursor(),
        &Bound::Included(vec![b'A']),
        &Bound::Unbounded,
    )
    .unwrap()
}

alphabet_tests! {
    bounds_cursor_start_bound_included: crate::bounds_cursor_start_bound_included,
}

fn bounds_cursor_start_bound_excluded(_: &str) -> BoundsCursor<ReferenceCursor, Error> {
    let mut builder = ReferenceBuilder::default();
    builder.put("@".as_bytes(), 0, "@".as_bytes()).unwrap();
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
    BoundsCursor::new(
        builder.seal().unwrap().cursor(),
        &Bound::Excluded(vec![b'@']),
        &Bound::Unbounded,
    )
    .unwrap()
}

alphabet_tests! {
    bounds_cursor_start_bound_excluded: crate::bounds_cursor_start_bound_excluded,
}

fn bounds_cursor_end_bound_included(_: &str) -> BoundsCursor<ReferenceCursor, Error> {
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
    builder.put("[".as_bytes(), 0, "[".as_bytes()).unwrap();
    BoundsCursor::new(
        builder.seal().unwrap().cursor(),
        &Bound::Unbounded,
        &Bound::Included(vec![b'Z']),
    )
    .unwrap()
}

alphabet_tests! {
    bounds_cursor_end_bound_included: crate::bounds_cursor_end_bound_included,
}

fn bounds_cursor_end_bound_excluded(_: &str) -> BoundsCursor<ReferenceCursor, Error> {
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
    builder.put("[".as_bytes(), 0, "[".as_bytes()).unwrap();
    BoundsCursor::new(
        builder.seal().unwrap().cursor(),
        &Bound::Unbounded,
        &Bound::Excluded(vec![b'[']),
    )
    .unwrap()
}

alphabet_tests! {
    bounds_cursor_end_bound_excluded: crate::bounds_cursor_end_bound_excluded,
}

fn bounds_cursor_both_bound_included(_: &str) -> BoundsCursor<ReferenceCursor, Error> {
    let mut builder = ReferenceBuilder::default();
    builder.put("@".as_bytes(), 0, "@".as_bytes()).unwrap();
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
    builder.put("[".as_bytes(), 0, "[".as_bytes()).unwrap();
    BoundsCursor::new(
        builder.seal().unwrap().cursor(),
        &Bound::Included(vec![b'A']),
        &Bound::Included(vec![b'Z']),
    )
    .unwrap()
}

alphabet_tests! {
    bounds_cursor_both_bound_included: crate::bounds_cursor_both_bound_included,
}

fn bounds_cursor_both_bound_excluded(_: &str) -> BoundsCursor<ReferenceCursor, Error> {
    let mut builder = ReferenceBuilder::default();
    builder.put("@".as_bytes(), 0, "@".as_bytes()).unwrap();
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
    builder.put("[".as_bytes(), 0, "[".as_bytes()).unwrap();
    BoundsCursor::new(
        builder.seal().unwrap().cursor(),
        &Bound::Excluded(vec![b'@']),
        &Bound::Excluded(vec![b'[']),
    )
    .unwrap()
}

alphabet_tests! {
    bounds_cursor_both_bound_excluded: crate::bounds_cursor_both_bound_excluded,
}
