use std::collections::{BTreeMap, BTreeSet};
use std::fs::{read_dir, read_to_string};
use std::path::PathBuf;

use semver::Version;
use toml::{Table, Value};

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
    dependency_names(&cargo_toml)
}

fn dependency_names(cargo_toml: &Table) -> Vec<String> {
    let mut deps = BTreeSet::new();
    collect_dependencies(cargo_toml, &mut deps);
    if let Some(targets) = cargo_toml.get("target").and_then(|v| v.as_table()) {
        for target in targets.values() {
            let target = target.as_table().expect("parsing target table");
            collect_dependencies(target, &mut deps);
        }
    }
    deps.into_iter().collect()
}

fn collect_dependencies(cargo_toml: &Table, deps: &mut BTreeSet<String>) {
    for heading in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(dependencies_table) = cargo_toml.get(heading).and_then(|v| v.as_table()) else {
            continue;
        };
        for (dependency, spec) in dependencies_table {
            deps.insert(dependency_name(dependency, spec));
        }
    }
}

fn dependency_name(dependency: &str, spec: &Value) -> String {
    spec.as_table()
        .and_then(|table| table.get("package"))
        .and_then(Value::as_str)
        .unwrap_or(dependency)
        .to_string()
}

pub fn graph() -> (Vec<String>, Vec<(String, String)>) {
    let vertices = workspace_members();
    let local_members = vertices
        .iter()
        .map(|member| {
            let package = package(member);
            (package.name, member.clone())
        })
        .collect::<BTreeMap<_, _>>();
    let mut edges = Vec::new();
    for member in &vertices {
        for dependency in dependencies(member) {
            if let Some(local_member) = local_members.get(&dependency) {
                edges.push((local_member.clone(), member.clone()));
            }
        }
    }
    (vertices, edges)
}

pub fn candidate_order() -> Vec<String> {
    let (vertices, edges) = graph();
    topological_order(vertices, edges)
}

fn topological_order(mut vertices: Vec<String>, mut edges: Vec<(String, String)>) -> Vec<String> {
    let mut in_sequence = Vec::new();
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
    use toml::Table;

    use super::{dependency_names, short_version, topological_order};

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

    #[test]
    fn dependency_names_include_all_dependency_kinds() {
        let manifest = r#"
[dependencies]
handled = "0.7"
renamed = { package = "zerror", version = "0.9" }

[dev-dependencies]
arrrg = "0.9"

[build-dependencies]
prototk = "0.14"

[target.'cfg(unix)'.dependencies]
buffertk = "0.14"

[target.'cfg(test)'.dev-dependencies]
sync42 = "0.16"
"#
        .parse::<Table>()
        .unwrap();
        assert_eq!(
            dependency_names(&manifest),
            vec![
                "arrrg".to_string(),
                "buffertk".to_string(),
                "handled".to_string(),
                "prototk".to_string(),
                "sync42".to_string(),
                "zerror".to_string(),
            ]
        );
    }

    #[test]
    fn topological_order_places_dependencies_first() {
        assert_eq!(
            topological_order(
                vec!["arrrg".to_string(), "buffertk".to_string()],
                vec![("arrrg".to_string(), "buffertk".to_string())],
            ),
            vec!["arrrg".to_string(), "buffertk".to_string()]
        );
    }
}
