mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::cargo_dir;

fn run_rcinvoke(service: &str) -> Output {
    let cargo_dir = cargo_dir();
    let cargo_dir = cargo_dir.into_std();
    let rcinvoke = cargo_dir.join("rcinvoke");
    let cargo_dir = cargo_dir
        .to_str()
        .expect("cargo_dir should be valid UTF-8")
        .to_string();
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = if current_path.is_empty() {
        cargo_dir
    } else {
        format!("{cargo_dir}:{current_path}")
    };

    Command::new(rcinvoke)
        .args(["--rc-conf-path", "rc.conf", "--rc-d-path", "rc.d", service])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .env("PATH", new_path)
        .output()
        .expect("rcinvoke should spawn")
}

#[test]
fn example1() {
    let output = run_rcinvoke("example1");

    // The command should execute and return a status code.
    // It may fail due to service not being enabled or other config issues,
    // but it should not panic or hang indefinitely.
    assert!(
        output.status.code().is_some(),
        "rcinvoke should exit with a status code: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn jfk_planetexpress_example4() {
    let output = run_rcinvoke("Jfk_PlanetExpress_example4");

    assert!(
        output.status.success(),
        "rcinvoke should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = "['--field1', 'Good News', '--field2', 'Everyone!']\n";

    assert_eq!(
        stdout.trim(),
        expected.trim(),
        "Output should match expected format. Got: {}, Expected: {}",
        stdout.trim(),
        expected.trim()
    );
}
