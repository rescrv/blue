use arrrg::CommandLine;

use lsmtk::LsmOptions;

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmtk-options [OPTIONS]");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    println!("{:#?}", options);
}
