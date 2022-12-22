use lp::block::BuilderOptions as BlockBuilderOptions;
use lp::table::{BlockCompression, Builder, BuilderOptions};
use lp::guacamole::fuzzer;

//fn new_table<T: TableTrait, B: TableBuilderTrait<Table=T>>() -> Box<B> {
fn new_table() -> Builder {
    println!("        let block_opts = BlockBuilderOptions::default()");
    println!("            .bytes_restart_interval:(512)");
    println!("            .key_value_pairs_restart_interval(16);");
    let block_opts = BlockBuilderOptions::default()
        .bytes_restart_interval(512)
        .key_value_pairs_restart_interval(16);
    println!("    let builder_opts = BuilderOptions::default()");
    println!("        .block_options(block_opts)");
    println!("        .block_compression(BlockCompression::NoCompression)");
    println!("        .target_block_size(4096);");
    let builder_opts = BuilderOptions::default()
        .block_options(block_opts)
        .block_compression(BlockCompression::NoCompression)
        .target_block_size(4096);
    println!("        let mut builder = Builder::new(\"lp-table-guacamole.sst\", builder_opts).unwrap();");
    let builder = Builder::new("lp-table-guacamole.sst", builder_opts).unwrap();
    builder
}

fn main() {
    fuzzer("lp-table-guacamole", "0.1", "Runs random workloads against lp::table.", new_table);
}
