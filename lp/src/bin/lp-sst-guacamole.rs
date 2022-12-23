use lp::block::BlockBuilderOptions as BlockBuilderOptions;
use lp::guacamole::fuzzer;
use lp::sst::{BlockCompression, SSTBuilder, SSTBuilderOptions};

fn new_table() -> SSTBuilder {
    println!("        let block_opts = BlockBuilderOptions::default()");
    println!("            .bytes_restart_interval:(512)");
    println!("            .key_value_pairs_restart_interval(16);");
    let block_opts = BlockBuilderOptions::default()
        .bytes_restart_interval(512)
        .key_value_pairs_restart_interval(16);
    println!("    let builder_opts = SSTBuilderOptions::default()");
    println!("        .block_options(block_opts)");
    println!("        .block_compression(BlockCompression::NoCompression)");
    println!("        .target_block_size(4096);");
    let builder_opts = SSTBuilderOptions::default()
        .block_options(block_opts)
        .block_compression(BlockCompression::NoCompression)
        .target_block_size(4096);
    println!("        let mut builder = SSTBuilder::new(\"lp-sst-guacamole.sst\".into(), builder_opts).unwrap();");
    let builder = SSTBuilder::new("lp-sst-guacamole.sst".into(), builder_opts).unwrap();
    builder
}

fn main() {
    fuzzer("lp-sst-guacamole", "0.1", "Runs random workloads against lp::sst.", new_table);
}
