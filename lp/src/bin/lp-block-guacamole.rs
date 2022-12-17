use lp::block::{Builder, BuilderOptions};
use lp::guacamole::fuzzer;

//fn new_table<T: TableTrait, B: TableBuilderTrait<Table=T>>() -> Box<B> {
fn new_table() -> Builder {
    let builder_opts = BuilderOptions::default()
        .bytes_restart_interval(512)
        .key_value_pairs_restart_interval(16);
    let builder = Builder::new(builder_opts);
    println!("        let builder_opts = BuilderOptions::default()");
    println!("            .bytes_restart_interval:(512)");
    println!("            .key_value_pairs_restart_interval(16);");
    println!("        let mut builder = Builder::new(builder_opts);");
    builder
}

fn main() {
    fuzzer("lp-block-guacamole", "0.1", "Runs random workloads against lp::block.", new_table);
}
