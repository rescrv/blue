use std::fs::read_to_string;
use std::path::PathBuf;

use toml::Table;

fn workspace_members() -> Vec<String> {
    let workspace_text = read_to_string("../Cargo.toml").expect("reading workspace toml");
    let workspace_toml = workspace_text.parse::<Table>().expect("parsing workspace toml");
    assert!(workspace_toml.contains_key("workspace"));
    let workspace_table = workspace_toml["workspace"].as_table().expect("parsing workspace table");
    assert!(workspace_table.contains_key("members"));
    let mut members = Vec::new();
    for member in workspace_table["members"].as_array().expect("parsing members array") {
        let member = member.as_str().expect("parsing member");
        members.push(member.to_string());
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

fn main() {
    let (mut vertices, mut edges) = graph();
    while !vertices.is_empty() {
        let mut candidates = Vec::new();
        for vertex in vertices.iter() {
            let num_inbound_edges = edges.iter().filter(|p| &p.1 == vertex).count();
            if num_inbound_edges == 0 {
                candidates.push(vertex);
            }
            println!("FINDME {}:{} {} => {:?}", file!(), line!(), vertex, num_inbound_edges);
        }
        candidates.sort();
        assert!(!candidates.is_empty());
        let candidate = candidates[0].to_owned();
        println!("{}", candidate);
        vertices.retain(|v| v != &candidate);
        edges.retain(|(src, dst)| src != &candidate);
    }
}
