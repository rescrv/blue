#[cfg(feature = "command_line")]
use arrrg::CommandLine;

pub mod ycsb;

#[cfg(feature = "command_line")]
use crate::{KeyValueStore, Workload};

//////////////////////////////////////////////// get ///////////////////////////////////////////////

#[cfg(feature = "command_line")]
pub fn from_command_line<KVS: KeyValueStore + 'static>(
    usage: &str,
    args: &[String],
) -> Box<dyn Workload<KVS>> {
    if args.is_empty() {
        eprintln!("no workload specified on the command line");
        std::process::exit(1);
    }
    let workload = &args[0];
    let args = args[1..].iter().map(String::as_str).collect::<Vec<&str>>();
    match workload.as_str() {
        "ycsb" => {
            let (options, free) = ycsb::WorkloadOptions::from_arguments_relaxed(usage, &args);
            if !free.is_empty() {
                eprintln!("ycsb workload takes no positional arguments");
                std::process::exit(1);
            }
            Box::new(ycsb::Workload::new(options)) as _
        }
        _ => {
            eprintln!("unknown workload: {}", workload);
            eprintln!("{}", usage);
            std::process::exit(1);
        }
    }
}
