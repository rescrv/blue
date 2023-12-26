use std::fs::{read_dir, read_to_string};
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Binary {
    name: String,
    path: String,
    #[serde(rename(deserialize = "required-features"))]
    required_features: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoToml {
    bin: Option<Vec<Binary>>,
    example: Option<Vec<Binary>>,
}

fn compare(dir: &str, what: &str, witnessed: Vec<String>) {
    if PathBuf::from(dir).exists() {
        for entry in read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path().to_string_lossy().to_string();
            if !witnessed.contains(&path) && path.ends_with(".rs") && !path.ends_with("/common.rs")
            {
                eprintln!("missing {} block for {}", what, path);
                std::process::exit(1);
            }
        }
    }
}

fn main() {
    let cargo_text = read_to_string("Cargo.toml").expect("reading Cargo.toml");
    let cargo_toml: CargoToml = toml::from_str(&cargo_text).unwrap();
    let mut witnessed = vec![];
    for bin in cargo_toml.bin.as_ref().unwrap_or(&vec![]).iter() {
        if !bin.required_features.contains(&"binaries".to_string()) {
            eprintln!("binary {} missing \"binaries\" feature", bin.name);
            std::process::exit(1);
        }
        witnessed.push(bin.path.clone());
    }
    compare("src/bin", "bin", witnessed);
    let mut witnessed = vec![];
    for example in cargo_toml.example.as_ref().unwrap_or(&vec![]).iter() {
        if !example.required_features.contains(&"binaries".to_string()) {
            eprintln!("example {} missing \"binaries\" feature", example.name);
            std::process::exit(1);
        }
        witnessed.push(example.path.clone());
    }
    compare("examples", "example", witnessed);
}
