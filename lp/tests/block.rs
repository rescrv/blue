extern crate lp;

use lp::block::{BlockBuilder, BlockBuilderOptions, BlockCursor};
use lp::Builder;

mod alphabet;
mod guacamole;

////////////////////////////////////////////// Options /////////////////////////////////////////////

fn opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1() -> BlockBuilderOptions {
    BlockBuilderOptions::default()
        .bytes_restart_interval(1)
        .key_value_pairs_restart_interval(1)
}

fn opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16() -> BlockBuilderOptions {
    BlockBuilderOptions::default()
        .bytes_restart_interval(512)
        .key_value_pairs_restart_interval(16)
}

////////////////////////////////////////// Alphabet Tests //////////////////////////////////////////

fn alphabet_opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1(_: &str) -> BlockCursor {
    alphabet(opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1())
}

alphabet_tests! {
    alphabet_opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1:
        crate::alphabet_opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1,
}

fn alphabet_opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16(_: &str) -> BlockCursor {
    alphabet(opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16())
}

alphabet_tests! {
    alphabet_opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16:
        crate::alphabet_opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16,
}

fn alphabet(opts: BlockBuilderOptions) -> BlockCursor {
    let mut builder = BlockBuilder::new(opts);
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
    builder.seal().unwrap().iterate()
}

///////////////////////////////////////////// Guacamole ////////////////////////////////////////////

fn guacamole_bytes_restart_interval_1_key_value_pairs_restart_interval_1(_: &str) -> BlockBuilder {
    BlockBuilder::new(opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1())
}

guacamole_tests! {
    guacamole_bytes_restart_interval_1_key_value_pairs_restart_interval_1:
        crate::guacamole_bytes_restart_interval_1_key_value_pairs_restart_interval_1,
}

fn guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16(_: &str) -> BlockBuilder {
    BlockBuilder::new(opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16())
}

guacamole_tests! {
    guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16:
        crate::guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16,
}
