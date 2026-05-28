use std::fs::{copy, rename};

use arrrg::CommandLine;
use rc_conf::RcConf;
use utf8path::Path;

use k8src::{RegenerateOptions, error_code, error_message, error_string_field};

fn top_level_help() {
    println!(
        "USAGE:
  k8src init [ROOT]
  k8src template service.yaml.template
  k8src edit rc.conf [rc.conf ...]
  k8src regenerate [OPTIONS]
  k8src explain-template [--root ROOT] <service>
  k8src explain-vars [--root ROOT] <service>

COMMANDS:
  init              Create rc.conf, service.yaml.template, rc.d/, pets/, .k8srcignore.
  template          Print a built-in template.
  edit              Edit and validate one or more rc.conf files.
  regenerate        Generate manifests under manifests/.
  explain-template  Show template selection and fallback chain.
  explain-vars      Show effective variables for a service.

EXAMPLES:
  k8src init
  k8src template service.yaml.template > service.yaml.template
  k8src regenerate --dry-run
  k8src regenerate --diff
  k8src explain-template memcached
  k8src explain-vars memcached

CANONICAL LAYOUT:
  /rc.conf
  /service.yaml.template
  /rc.d/<service>.yaml.template
  /pets/...
"
    )
}

fn parse_root_and_service(args: &[String], usage: &str) -> (Option<String>, String) {
    let mut root = None;
    let mut service = None;
    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                if idx >= args.len() {
                    eprintln!("{usage}");
                    eprintln!("missing value for --root");
                    std::process::exit(254);
                }
                root = Some(args[idx].clone());
            }
            arg if arg.starts_with("--root=") => {
                root = Some(arg["--root=".len()..].to_string());
            }
            arg if arg.starts_with('-') => {
                eprintln!("{usage}");
                eprintln!("unknown option {arg}");
                std::process::exit(254);
            }
            arg => {
                if service.is_some() {
                    eprintln!("{usage}");
                    eprintln!("too many positional arguments");
                    std::process::exit(254);
                }
                service = Some(arg.to_string());
            }
        }
        idx += 1;
    }
    let Some(service) = service else {
        eprintln!("{usage}");
        eprintln!("missing service");
        std::process::exit(254);
    };
    (root, service)
}

fn report_error(command: &str, err: &k8src::Error) {
    let message = error_message(err).unwrap_or_else(|| err.to_string());
    eprintln!("{command} error: {message}");
    if let Some(code) = error_code(err) {
        eprintln!("  code: {code}");
    }
    for field in [
        "service",
        "template",
        "rc_conf_path",
        "output",
        "path",
        "relative",
        "key",
        "context",
        "operation",
        "rc_command",
        "working_directory",
        "exit_status",
        "stdout",
        "stderr",
    ] {
        if let Some(value) = error_string_field(err, field)
            && !value.is_empty()
        {
            eprintln!("  {field}: {value}");
        }
    }
}

fn init(root: &str) {
    let root = Path::from(root);
    for dir in [root.join("rc.d"), root.join("pets")] {
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("could not create {}: {err}", dir.as_str());
            std::process::exit(252);
        }
    }
    let files = [
        (
            root.join("rc.conf"),
            r#"NAMESPACE="default"
example_ENABLED="YES"
example_IMAGE="example:latest"
example_PORT="8080"
"#
            .to_string(),
        ),
        (
            root.join("service.yaml.template"),
            k8src::default_service_template().to_string(),
        ),
        (
            root.join(".k8srcignore"),
            "# Place this file in generated or external directories k8src should skip.\n"
                .to_string(),
        ),
    ];
    for (path, contents) in files {
        if path.exists().unwrap_or(false) {
            eprintln!("exists: {}", path.as_str());
            continue;
        }
        if let Err(err) = std::fs::write(&path, contents) {
            eprintln!("could not write {}: {err}", path.as_str());
            std::process::exit(252);
        }
        println!("created {}", path.as_str());
    }
}

fn edit(rc_conf: &str) {
    let editor = std::env::var("EDITOR").unwrap_or("nano".to_string());
    let tmpfile = rc_conf.to_string() + ".tmp";
    match Path::from(&tmpfile).exists() {
        Ok(true) => {
            eprintln!("erase {tmpfile} and try again");
            std::process::exit(253);
        }
        Ok(false) => {}
        Err(err) => {
            eprintln!("could not inspect tempfile: {err}");
            std::process::exit(253);
        }
    }
    if let Err(err) = copy(rc_conf, &tmpfile) {
        eprintln!("could not copy to tempfile: {err}");
        std::process::exit(252);
    }
    let status = match std::process::Command::new(&editor)
        .args([&tmpfile])
        .status()
    {
        Ok(status) => status,
        Err(err) => {
            eprintln!("{editor} failed to spawn; is it in PATH");
            eprintln!("error: {err}");
            std::process::exit(251);
        }
    };
    if Some(0) == status.code() {
        let contents = match RcConf::examine(&tmpfile) {
            Ok(contents) => contents,
            Err(err) => {
                eprintln!("could not parse rc_conf: {err:?}");
                std::process::exit(249);
            }
        };
        if let Err(err) = rename(tmpfile, rc_conf) {
            eprintln!("could not rename tempfile: {err}");
            std::process::exit(250);
        }
        println!("{}", contents.trim());
    } else {
        eprintln!("{editor} failed to edit; see above for an error");
        std::process::exit(248);
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() <= 1 {
        top_level_help();
        return;
    }
    match args[1].as_str() {
        "help" => {
            top_level_help();
        }
        "template" => {
            if args.len() != 3 {
                eprintln!("template command takes exactly one argument: the template");
                std::process::exit(254);
            } else if args[2] == "service.yaml.template" {
                println!("{}", k8src::default_service_template().trim());
            } else {
                eprintln!("unknown template {}", args[2]);
                eprintln!("valid templates:");
                eprintln!("- service.yaml.template");
                std::process::exit(254);
            }
        }
        "init" => {
            if args.len() > 3 {
                eprintln!("USAGE: k8src init [ROOT]");
                std::process::exit(254);
            }
            init(args.get(2).map(String::as_str).unwrap_or("."));
        }
        "edit" => {
            if args.len() <= 2 {
                eprintln!("edit requires at least one rc_conf path");
                std::process::exit(254);
            }
            for rc_conf in args[2..].iter() {
                edit(rc_conf);
            }
        }
        "regenerate" => {
            let args = args.iter().map(|a| a.as_str()).collect::<Vec<_>>();
            let (options, free) = RegenerateOptions::from_arguments_relaxed(
                "USAGE: k8src regenerate [OPTIONS]",
                &args[2..],
            );
            if !free.is_empty() {
                eprintln!("regenerate takes no positional arguments");
                std::process::exit(247);
            }
            if let Err(err) = k8src::regenerate(options) {
                report_error("regenerate", &err);
                if std::env::var("RUST_BACKTRACE").is_ok() {
                    eprintln!("{err:?}");
                }
                std::process::exit(246);
            }
        }
        "explain-template" => {
            let (root, service) = parse_root_and_service(
                &args[2..],
                "USAGE: k8src explain-template [--root ROOT] <service>",
            );
            match k8src::explain_template(root.as_deref(), &service) {
                Ok(explanation) => print!("{explanation}"),
                Err(err) => {
                    report_error("explain-template", &err);
                    std::process::exit(246);
                }
            }
        }
        "explain-vars" => {
            let (root, service) = parse_root_and_service(
                &args[2..],
                "USAGE: k8src explain-vars [--root ROOT] <service>",
            );
            match k8src::explain_vars(root.as_deref(), &service) {
                Ok(explanation) => print!("{explanation}"),
                Err(err) => {
                    report_error("explain-vars", &err);
                    std::process::exit(246);
                }
            }
        }
        _ => {
            eprintln!("unknown command {}\n", args[1]);
            top_level_help();
            std::process::exit(255);
        }
    }
}
