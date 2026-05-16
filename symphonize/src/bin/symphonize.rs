use arrrg::CommandLine;

use rc_conf::RcConf;

use symphonize::{Error, Symphonize, SymphonizeOptions, autoinfer_configuration, paths_to_root};

fn usage() {
    eprintln!(
        "{}",
        "USAGE: symphonize command

list of supported commands:
debug
apply
build-images
build-manifests
apply-manifests
"
        .trim()
    );
}

fn main() {
    if let Err(err) = run() {
        eprintln!("symphonize: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let (options, free) = SymphonizeOptions::from_command_line("USAGE: symphonize [command]");
    if free.len() != 1 {
        usage();
        std::process::exit(255);
    }
    let paths_to_root = paths_to_root(&options)?;
    let rc_conf_path = autoinfer_configuration(&options, &paths_to_root)?;
    let rc_conf = RcConf::parse(&rc_conf_path)?;
    match free[0].as_str() {
        "debug" => {
            println!("RC_CONF_PATH={rc_conf_path}");
            println!("RC_CONF={rc_conf:#?}");
        }
        "apply" => {
            let mut symphonize = Symphonize::new(options, paths_to_root[0].clone(), rc_conf);
            symphonize.apply()?;
        }
        "build-images" => {
            let mut symphonize = Symphonize::new(options, paths_to_root[0].clone(), rc_conf);
            symphonize.build_images()?;
        }
        "build-manifests" => {
            let mut symphonize = Symphonize::new(options, paths_to_root[0].clone(), rc_conf);
            symphonize.build_manifests()?;
        }
        "apply-manifests" => {
            let mut symphonize = Symphonize::new(options, paths_to_root[0].clone(), rc_conf);
            symphonize.apply_manifests()?;
        }
        _ => {
            usage();
            std::process::exit(255);
        }
    }
    Ok(())
}
