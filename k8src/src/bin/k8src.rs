use std::collections::HashMap;
use std::fs::{copy, rename};

use arrrg::CommandLine;
use rc_conf::RcConf;
use utf8path::Path;

use k8src::RegenerateOptions;

const TEMPLATES: &[(&str, &str)] = &[(
    "service.yaml.template",
    r#"
"#,
)];

fn top_level_help() {
    eprintln!(
        "USAGE: k8src template <template_name.yaml.template>
       k8src edit rc.conf
       k8src regenerate
"
    )
}

fn edit(rc_conf: &str) {
    let editor = std::env::var("EDITOR").unwrap_or("nano".to_string());
    let tmpfile = rc_conf.to_string() + ".tmp";
    if Path::from(&tmpfile).exists() {
        eprintln!("erase {tmpfile} and try again");
        std::process::exit(253);
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
    let templates: HashMap<String, String> = HashMap::from_iter(
        TEMPLATES
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string())),
    );
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
            } else if let Some(template) = templates.get(args[2].as_str()) {
                println!("{}", template.trim());
            } else {
                eprintln!("unknown template {}", args[2]);
                eprintln!("valid templates:");
                for template in templates.keys() {
                    eprintln!("- {template}");
                }
                std::process::exit(254);
            }
        }
        "edit" => {
            for rc_conf in args[2..].iter() {
                edit(rc_conf);
            }
        }
        "regenerate" => {
            let args = args.iter().map(|a| a.as_str()).collect::<Vec<_>>();
            let (options, free) =
                RegenerateOptions::from_arguments("USAGE: k8src regenerate [OPTIONS]", &args[2..]);
            if !free.is_empty() {
                eprintln!("command takes no positional arguments");
                std::process::exit(247);
            }
            if let Err(err) = k8src::regenerate(options) {
                eprintln!("regenerate error: {err:?}");
                std::process::exit(246);
            }
        }
        _ => {
            eprintln!("unknown command {}\n", args[1]);
            top_level_help();
            std::process::exit(255);
        }
    }
}
