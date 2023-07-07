use std::process::{Command, Output, Stdio};

use serde::{Deserialize, Serialize};

//////////////////////////////////////////// JSON Types ////////////////////////////////////////////

#[derive(Debug, Serialize, Deserialize)]
struct Reason {
    reason: String,
}

fn reason(x: &str) -> String {
    let reason: Reason = serde_json::from_str(x).expect("could not parse reason");
    reason.reason
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildFinished {
    reason: String,
    success: bool
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildScriptExecuted {
    reason: String,
    package_id: String,
    cfgs: Vec<String>,
    linked_libs: Vec<String>,
    linked_paths: Vec<String>,
    env: Vec<(String, String)>,
    out_dir: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    kind: Vec<String>,
    crate_types: Vec<String>,
    name: String,
    src_path: String,
    edition: String,
    doc: bool,
    doctest: bool,
    test: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    opt_level: String,
    debuginfo: Option<u64>,
    debug_assertions: bool,
    overflow_checks: bool,
    test: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerArtifact {
    reason: String,
    package_id: String,
    manifest_path: String,
    target: Target,
    profile: Profile,
    features: Vec<String>,
    filenames: Vec<String>,
    executable: Option<String>,
    fresh: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    rendered: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerMessage {
    reason: String,
    package_id: String,
    manifest_path: String,
    target: Target,
    message: Message,
}

/////////////////////////////////////////// handle_output //////////////////////////////////////////

fn handle_output(output: Output) {
    let line = std::str::from_utf8(&output.stdout).expect("could not parse output of \"cargo check\"");
    for line in line.split_terminator('\n') {
        if line.starts_with("test") {
            println!("{}", line);
            continue;
        }
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let reason = reason(line);
        if reason == "build-finished" {
            let _x: BuildFinished = serde_json::from_str(line).expect("could not parse BuildFinished");
        } else if reason == "build-script-executed" {
            let _x: BuildScriptExecuted = serde_json::from_str(line).expect("could not parse BuildScriptExecuted");
        } else if reason == "compiler-artifact" {
            let _x: CompilerArtifact = serde_json::from_str(line).expect("could not parse CompilerArtifact");
        } else if reason == "compiler-message" {
            let x: CompilerMessage = serde_json::from_str(line).expect("could not parse CompilerMessage");
            println!("{}", x.message.rendered);
        } else {
            panic!("unhandled check case");
        }
    }
}

////////////////////////////////////////// Cargo Commands //////////////////////////////////////////

fn cargo_fetch() {
    println!("┏━━━━━━━━━━━━━┓");
    println!("┃ cargo fetch ┃");
    println!("┗━━━━━━━━━━━━━┛");
    let mut proc = Command::new("cargo")
        .args(["fetch", "--quiet"])
        .spawn()
        .expect("failed to execute \"cargo fetch\"");
    proc.wait().expect("failed to wait for \"cargo fetch\"");
}

fn cargo_check() {
    println!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
    println!("┃ cargo check --frozen --offline --message-format json --workspace --quiet ┃");
    println!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
    let proc = Command::new("cargo")
        .args(["check", "--frozen", "--offline", "--message-format", "json", "--workspace", "--quiet"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute \"cargo check\"");
    let output = proc.wait_with_output().expect("failed to wait for \"cargo check\"");
    handle_output(output);
}

fn cargo_test_lib() {
    println!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
    println!("┃ cargo test --frozen --offline --message-format json --workspace --lib ┃");
    println!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
    let proc = Command::new("cargo")
        .args(["test", "--frozen", "--offline", "--message-format", "json", "--workspace", "--lib"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute \"cargo test --lib\"");
    let output = proc.wait_with_output().expect("failed to wait for \"cargo test --lib\"");
    handle_output(output);
}

fn cargo_test_all() {
    println!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
    println!("┃ cargo test --frozen --offline --message-format json --workspace --all-targets ┃");
    println!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
    let proc = Command::new("cargo")
        .args(["test", "--frozen", "--offline", "--message-format", "json", "--workspace", "--all-targets"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute \"cargo test --all-targets\"");
    let output = proc.wait_with_output().expect("failed to wait for \"cargo test --all-targets\"");
    handle_output(output);
}

fn cargo_build() {
    println!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
    println!("┃ cargo build --frozen --offline --message-format json --workspace --all-targets ┃");
    println!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
    let proc = Command::new("cargo")
        .args(["build", "--frozen", "--offline", "--message-format", "json", "--workspace", "--all-targets"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute \"cargo build\"");
    let output = proc.wait_with_output().expect("failed to wait for \"cargo build\"");
    handle_output(output);
}

fn cargo_doc() {
    println!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
    println!("┃ cargo doc --frozen --offline --message-format json --workspace --no-deps ┃");
    println!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
    let proc = Command::new("cargo")
        .args(["doc", "--frozen", "--offline", "--message-format", "json", "--workspace", "--no-deps"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute \"cargo doc\"");
    let output = proc.wait_with_output().expect("failed to wait for \"cargo doc\"");
    handle_output(output);
}

fn cargo_clippy() {
    println!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
    println!("┃ cargo clippy --frozen --offline --message-format json --workspace ┃");
    println!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
    let proc = Command::new("cargo")
        .args(["clippy", "--frozen", "--offline", "--message-format", "json", "--workspace"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute \"cargo clippy\"");
    let output = proc.wait_with_output().expect("failed to wait for \"cargo clippy\"");
    handle_output(output);
}

fn main() {
    cargo_fetch();
    cargo_check();
    cargo_test_lib();
    cargo_test_all();
    cargo_build();
    cargo_doc();
    cargo_clippy();
    println!("Success!\nThis concludes our barrage of cargo commands.");
}
