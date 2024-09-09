use arrrg::CommandLine;

use rc_conf::RcConf;

use symphonize::{autoinfer_configuration, paths_to_root, Symphonize, SymphonizeOptions};

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
    let (options, free) = SymphonizeOptions::from_command_line("USAGE: symphonize [command]");
    if free.is_empty() {
        usage();
        std::process::exit(255);
    }
    let paths_to_root = paths_to_root(&options).expect("should be able to compute paths to root");
    let rc_conf_path = autoinfer_configuration(&options, &paths_to_root)
        .expect("should be able to auto-infer configuration");
    let rc_conf = RcConf::parse(&rc_conf_path).expect("should be able to parse rc.conf");
    let mut symphonize = Symphonize::new(options, paths_to_root[0].clone(), rc_conf.clone());
    match free[0].as_str() {
        "debug" => {
            println!("RC_CONF_PATH={}", rc_conf_path);
            println!("RC_CONF={:#?}", rc_conf);
        }
        "apply" => symphonize.apply().expect("apply should succeed"),
        "build-images" => symphonize
            .build_images()
            .expect("build-images should succeed"),
        "build-manifests" => symphonize
            .build_manifests()
            .expect("build-manifests should succeed"),
        "apply-manifests" => symphonize
            .apply_manifests()
            .expect("apply-manifests should succeed"),
        _ => {
            usage();
            std::process::exit(255);
        }
    }
}
