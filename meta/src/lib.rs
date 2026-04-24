use std::fs::{read_dir, read_to_string};
use std::path::PathBuf;

use semver::Version;
use toml::Table;

pub const EXCLUDE_MEMBERS: &[&str] = &["libpaxos", "meta", "paxos_pb"];

pub const EXCLUDE_DIRS: &[&str] = &[
    ".git",
    ".github",
    ".claude",
    "target",
    "napkins",
    ".symphonize",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    pub member: String,
    pub name: String,
    pub version: Version,
    pub manifest_path: PathBuf,
}

pub fn workspace_members() -> Vec<String> {
    let workspace_text = read_to_string("Cargo.toml").expect("reading workspace toml");
    let workspace_toml = workspace_text
        .parse::<Table>()
        .expect("parsing workspace toml");
    let workspace_table = workspace_toml["workspace"]
        .as_table()
        .expect("parsing workspace table");
    let mut members = Vec::new();
    for member in workspace_table["members"]
        .as_array()
        .expect("parsing members array")
    {
        let member = member.as_str().expect("parsing member");
        if !EXCLUDE_MEMBERS.iter().any(|x| x == &member) {
            members.push(member.to_string());
        }
    }
    for path in read_dir(".").expect("reading workspace directory") {
        let path = path.expect("reading directory entry");
        let name = path.file_name();
        if !members.iter().any(|x| x == &name.to_string_lossy())
            && path.path().is_dir()
            && !EXCLUDE_MEMBERS.iter().any(|x| x == &name.to_string_lossy())
            && !EXCLUDE_DIRS.iter().any(|x| x == &name.to_string_lossy())
        {
            panic!(
                "\"{}\" not in manifest and not excluded",
                path.file_name().to_string_lossy()
            );
        }
    }
    members.sort();
    members
}

pub fn package(member: &str) -> Package {
    let manifest_path = PathBuf::from(".").join(member).join("Cargo.toml");
    let cargo_text = read_to_string(&manifest_path).expect("reading cargo toml");
    let cargo_toml = cargo_text.parse::<Table>().expect("parsing cargo toml");
    let package = cargo_toml["package"]
        .as_table()
        .expect("parsing package table");
    let name = package["name"]
        .as_str()
        .expect("parsing package name")
        .to_string();
    let version = package["version"]
        .as_str()
        .expect("parsing package version")
        .parse()
        .expect("parsing semver version");
    Package {
        member: member.to_string(),
        name,
        version,
        manifest_path,
    }
}

pub fn dependencies(member: &str) -> Vec<String> {
    let cargo_text = read_to_string(PathBuf::from(".").join(member).join("Cargo.toml"))
        .expect("reading cargo toml");
    let cargo_toml = cargo_text.parse::<Table>().expect("parsing cargo toml");
    let Some(dependencies_table) = cargo_toml.get("dependencies").and_then(|v| v.as_table()) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for dep in dependencies_table {
        deps.push(dep.0.to_string());
    }
    deps
}

pub fn graph() -> (Vec<String>, Vec<(String, String)>) {
    let vertices = workspace_members();
    let mut edges = Vec::new();
    for member in &vertices {
        for dependency in dependencies(member) {
            if PathBuf::from(".").join(&dependency).is_dir() {
                edges.push((dependency, member.clone()));
            }
        }
    }
    (vertices, edges)
}

pub fn candidate_order() -> Vec<String> {
    let mut in_sequence = Vec::new();
    let (mut vertices, mut edges) = graph();
    while !vertices.is_empty() {
        let mut candidates = Vec::new();
        for vertex in &vertices {
            let num_inbound_edges = edges.iter().filter(|(_, dst)| dst == vertex).count();
            if num_inbound_edges == 0 {
                candidates.push(vertex.clone());
            }
        }
        candidates.sort();
        let candidate = candidates
            .first()
            .expect("acyclic dependency graph")
            .to_owned();
        vertices.retain(|v| v != &candidate);
        edges.retain(|(src, _)| src != &candidate);
        in_sequence.push(candidate);
    }
    in_sequence
}

pub fn short_version(version: &Version) -> String {
    if version.major > 0 {
        format!("{}.{}", version.major, version.minor)
    } else if version.minor > 0 {
        format!("0.{}", version.minor)
    } else {
        format!("0.0.{}", version.patch)
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::short_version;

    #[test]
    fn short_version_for_stable_series() {
        assert_eq!(short_version(&Version::parse("1.2.3").unwrap()), "1.2");
    }

    #[test]
    fn short_version_for_zero_major_series() {
        assert_eq!(short_version(&Version::parse("0.13.4").unwrap()), "0.13");
    }

    #[test]
    fn short_version_for_zero_zero_series() {
        assert_eq!(short_version(&Version::parse("0.0.7").unwrap()), "0.0.7");
    }
}
