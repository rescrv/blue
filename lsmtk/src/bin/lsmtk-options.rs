use arrrg::CommandLine;

use lsmtk::LsmtkOptions;

fn main() {
    let (options, free) = LsmtkOptions::from_command_line("USAGE: lsmtk-options [OPTIONS]");
    if !free.is_empty() {
        eprintln!("expected no positional arguments");
        std::process::exit(1);
    }
    println!("{:#?}", options);
}
