use lp::block::{BlockBuilder, BlockBuilderOptions};
use lp::guacamole::fuzzer;

fn new_table() -> BlockBuilder {
    let builder_opts = BlockBuilderOptions::default()
        .bytes_restart_interval(512)
        .key_value_pairs_restart_interval(16);
    let builder = BlockBuilder::new(builder_opts);
    println!("        let builder_opts = BlockBuilderOptions::default()");
    println!("            .bytes_restart_interval:(512)");
    println!("            .key_value_pairs_restart_interval(16);");
    println!("        let mut builder = BlockBuilder::new(builder_opts);");
    builder
}

fn main() {
    fuzzer("lp-block-guacamole", "0.1", "Runs random workloads against lp::block.", new_table);
}
