extern crate sst;

use std::fs::remove_file;
use std::path::PathBuf;

use sst::block::BlockBuilderOptions;
use sst::{BlockCompression, Builder, SstBuilder, SstCursor, SstOptions};

mod alphabet;
mod guacamole;

////////////////////////////////////////////// Options /////////////////////////////////////////////

fn opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096(
) -> SstOptions {
    let builder_opts = BlockBuilderOptions::default()
        .bytes_restart_interval(1)
        .key_value_pairs_restart_interval(1);
    SstOptions::default()
        .block(builder_opts)
        .block_compression(BlockCompression::NoCompression)
        .target_block_size(4096)
}

fn opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096(
) -> SstOptions {
    let builder_opts = BlockBuilderOptions::default()
        .bytes_restart_interval(512)
        .key_value_pairs_restart_interval(16);
    SstOptions::default()
        .block(builder_opts)
        .block_compression(BlockCompression::NoCompression)
        .target_block_size(4096)
}

fn opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_65536(
) -> SstOptions {
    let builder_opts = BlockBuilderOptions::default()
        .bytes_restart_interval(512)
        .key_value_pairs_restart_interval(16);
    SstOptions::default()
        .block(builder_opts)
        .block_compression(BlockCompression::NoCompression)
        .target_block_size(65536)
}

////////////////////////////////////////// Alphabet Tests //////////////////////////////////////////

fn alphabet_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096(
    test: &str,
) -> SstCursor {
    alphabet(test, opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096())
}

alphabet_tests! {
    alphabet_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096:
        crate::alphabet_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096,
}

fn alphabet_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096(
    test: &str,
) -> SstCursor {
    alphabet(test, opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096())
}

alphabet_tests! {
    alphabet_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096:
        crate::alphabet_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096,
}

fn alphabet(test: &str, builder_opts: SstOptions) -> SstCursor {
    let s = test.to_string() + ".sst";
    let path: PathBuf = s.into();
    let mut builder = SstBuilder::new(builder_opts, path.clone()).unwrap();
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
    let cursor = builder.seal().unwrap().cursor();
    remove_file(path).unwrap();
    cursor
}

///////////////////////////////////////////// Guacamole ////////////////////////////////////////////

fn guacamole_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096(
    test: &str,
) -> SstBuilder {
    let path: PathBuf = (test.to_string() + ".sst").into();
    remove_file(path.clone()).err();
    let builder = SstBuilder::new(opts_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096(), path).unwrap();
    builder
}

guacamole_tests! {
    guacamole_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096:
        crate::guacamole_bytes_restart_interval_1_key_value_pairs_restart_interval_1_uncompressed_target_block_size_4096,
}

fn guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096(
    test: &str,
) -> SstBuilder {
    let path: PathBuf = (test.to_string() + ".sst").into();
    remove_file(path.clone()).err();
    let builder = SstBuilder::new(opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096(), path).unwrap();
    builder
}

guacamole_tests! {
    guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096:
        crate::guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_4096,
}

fn guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_65536(
    test: &str,
) -> SstBuilder {
    let path: PathBuf = (test.to_string() + ".sst").into();
    remove_file(path.clone()).err();
    let builder = SstBuilder::new(opts_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_65536(), path).unwrap();
    builder
}

guacamole_tests! {
    guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_65536:
        crate::guacamole_bytes_restart_interval_512_key_value_pairs_restart_interval_16_uncompressed_target_block_size_65536,
}
