use std::fs::{read_dir, read_to_string};
use std::path::PathBuf;

use toml::Table;

const EXCLUDE_MEMBERS: &[&str] = &[
    "meta",
];

const EXCLUDE_DIRS: &[&str] = &[
    ".git",
    "target",
    // TODO(rescrv): Stuff to integrate eventually.
    "statslicer",
];

fn workspace_members() -> Vec<String> {
    let workspace_text = read_to_string("../Cargo.toml").expect("reading workspace toml");
    let workspace_toml = workspace_text.parse::<Table>().expect("parsing workspace toml");
    assert!(workspace_toml.contains_key("workspace"));
    let workspace_table = workspace_toml["workspace"].as_table().expect("parsing workspace table");
    assert!(workspace_table.contains_key("members"));
    let mut members = Vec::new();
    for member in workspace_table["members"].as_array().expect("parsing members array") {
        let member = member.as_str().expect("parsing member");
        if !EXCLUDE_MEMBERS.iter().any(|x| x == &member) {
            members.push(member.to_string());
        }
    }
    for path in read_dir("..").unwrap() {
        let path = path.unwrap();
        let name = path.file_name();
        if !members.iter().any(|x| x == &name.to_string_lossy())
        && path.path().is_dir()
        && !EXCLUDE_MEMBERS.iter().any(|x| x == &name.to_string_lossy())
        && !EXCLUDE_DIRS.iter().any(|x| x == &name.to_string_lossy())
        {
            panic!("\"{}\" not in manifest and not excluded", path.file_name().to_string_lossy());
        }
    }
    members
}

fn dependencies(member: &str) -> Vec<String> {
    let cargo_text = read_to_string(PathBuf::from("..").join(member).join("Cargo.toml")).expect("reading cargo toml");
    let cargo_toml = cargo_text.parse::<Table>().expect("parsing cargo toml");
    assert!(cargo_toml.contains_key("dependencies"));
    let dependencies_table = cargo_toml["dependencies"].as_table().expect("parsing dependencies");
    let mut deps = Vec::new();
    for dep in dependencies_table {
        deps.push(dep.0.to_string());
    }
    deps
}

fn graph() -> (Vec<String>, Vec<(String, String)>) {
    let vertices = workspace_members();
    let mut edges = Vec::new();
    for member in vertices.iter() {
        for dependency in dependencies(member).into_iter() {
            if PathBuf::from("..").join(&dependency).is_dir() {
                edges.push((dependency, member.clone()));
            }
        }
    }
    (vertices, edges)
}

fn candidate_order() -> Vec<String> {
    let mut in_sequence = Vec::new();
    let (mut vertices, mut edges) = graph();
    while !vertices.is_empty() {
        let mut candidates = Vec::new();
        for vertex in vertices.iter() {
            let num_inbound_edges = edges.iter().filter(|p| &p.1 == vertex).count();
            if num_inbound_edges == 0 {
                candidates.push(vertex);
            }
        }
        candidates.sort();
        let candidate = candidates[0].to_owned();
        vertices.retain(|v| v != &candidate);
        edges.retain(|(src, _)| src != &candidate);
        in_sequence.push(candidate);
    }
    in_sequence
}

fn main() {
    let candidates = candidate_order();
    println!("publication order:");
    for candidate in candidates.iter() {
        println!("{}", candidate);
    }
}
