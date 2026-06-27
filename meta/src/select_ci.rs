use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

pub const DEFAULT_GLOBAL_TRIGGER_PATHS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "ci",
    ".github/workflows/rust.yml",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionMode {
    Full,
    Scoped,
}

impl SelectionMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Scoped => "scoped",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageSelection {
    pub member: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FullTrigger {
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub mode: SelectionMode,
    pub direct_packages: Vec<PackageSelection>,
    pub selected_packages: Vec<PackageSelection>,
    pub full_triggers: Vec<FullTrigger>,
}

impl Selection {
    pub fn write_facts(&self, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut facts = String::new();
        facts.push_str(&format!("mode\t{}\n", self.mode.as_str()));
        for package in &self.direct_packages {
            facts.push_str(&format!("direct_member\t{}\n", package.member));
            facts.push_str(&format!("direct_package\t{}\n", package.name));
        }
        for package in &self.selected_packages {
            facts.push_str(&format!("selected_member\t{}\n", package.member));
            facts.push_str(&format!("selected_package\t{}\n", package.name));
        }
        for trigger in &self.full_triggers {
            facts.push_str(&format!(
                "full_trigger\t{}\t{}\n",
                trigger.path, trigger.reason
            ));
        }
        fs::write(output, facts)?;
        Ok(())
    }

    pub fn print_report(&self) {
        println!("CI SELECTION");
        println!("  mode: {}", self.mode.as_str());
        if !self.full_triggers.is_empty() {
            println!("  full rebuild triggers:");
            for trigger in &self.full_triggers {
                println!("    {} ({})", trigger.path, trigger.reason);
            }
        }
        print_packages("direct packages", &self.direct_packages);
        if self.mode == SelectionMode::Scoped {
            print_packages("selected packages", &self.selected_packages);
        }
    }
}

fn print_packages(label: &str, packages: &[PackageSelection]) {
    println!("  {label}:");
    if packages.is_empty() {
        println!("    <none>");
    } else {
        for package in packages {
            println!("    {} ({})", package.name, package.member);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspacePackage {
    pub member: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Workspace {
    packages: BTreeMap<String, WorkspacePackage>,
    member_order: Vec<String>,
    reverse_dependencies: BTreeMap<String, BTreeSet<String>>,
}

impl Workspace {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--no-deps"])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
            .into());
        }
        Self::from_metadata_json(&output.stdout)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e).into())
    }

    pub fn from_metadata_json(metadata_json: &[u8]) -> Result<Self, String> {
        let metadata: CargoMetadata =
            serde_json::from_slice(metadata_json).map_err(|e| format!("parsing metadata: {e}"))?;
        let workspace_root = PathBuf::from(&metadata.workspace_root);
        let workspace_members = metadata
            .workspace_members
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut packages = BTreeMap::new();
        let mut member_by_package_root = BTreeMap::new();

        for package in &metadata.packages {
            if !workspace_members.contains(&package.id) {
                continue;
            }
            let manifest_path = PathBuf::from(&package.manifest_path);
            let package_root = manifest_path
                .parent()
                .ok_or_else(|| format!("manifest has no parent: {}", package.manifest_path))?;
            let member_path = package_root.strip_prefix(&workspace_root).map_err(|_| {
                format!(
                    "manifest {} is not under workspace root {}",
                    package.manifest_path, metadata.workspace_root
                )
            })?;
            let member = path_to_repo_string(member_path);
            if member.is_empty() {
                return Err(format!(
                    "workspace package {} unexpectedly lives at the workspace root",
                    package.name
                ));
            }
            packages.insert(
                member.clone(),
                WorkspacePackage {
                    member: member.clone(),
                    name: package.name.clone(),
                },
            );
            member_by_package_root.insert(path_to_repo_string(package_root), member);
        }

        let mut reverse_dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        for package in &metadata.packages {
            if !workspace_members.contains(&package.id) {
                continue;
            }
            let manifest_path = PathBuf::from(&package.manifest_path);
            let Some(package_root) = manifest_path.parent() else {
                continue;
            };
            let Some(member) = member_by_package_root
                .get(&path_to_repo_string(package_root))
                .cloned()
            else {
                continue;
            };
            for dependency in &package.dependencies {
                let Some(dependency_path) = dependency.path.as_ref() else {
                    continue;
                };
                if let Some(dependency_member) =
                    member_by_package_root.get(&path_to_repo_string(Path::new(dependency_path)))
                {
                    reverse_dependencies
                        .entry(dependency_member.clone())
                        .or_default()
                        .insert(member.clone());
                }
            }
        }

        let member_order =
            topological_order(packages.keys().cloned().collect(), &reverse_dependencies)?;
        Ok(Self {
            packages,
            member_order,
            reverse_dependencies,
        })
    }

    pub fn from_parts(
        packages: Vec<WorkspacePackage>,
        reverse_edges: Vec<(String, String)>,
    ) -> Result<Self, String> {
        let packages = packages
            .into_iter()
            .map(|package| (package.member.clone(), package))
            .collect::<BTreeMap<_, _>>();
        let mut reverse_dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        for (dependency, dependent) in reverse_edges {
            reverse_dependencies
                .entry(dependency)
                .or_default()
                .insert(dependent);
        }
        let member_order =
            topological_order(packages.keys().cloned().collect(), &reverse_dependencies)?;
        Ok(Self {
            packages,
            member_order,
            reverse_dependencies,
        })
    }

    fn owning_member(&self, path: &str) -> Option<String> {
        self.packages
            .keys()
            .filter(|member| path == member.as_str() || path.starts_with(&format!("{member}/")))
            .max_by_key(|member| member.len())
            .cloned()
    }

    fn package_selection(&self, member: &str) -> PackageSelection {
        let package = self
            .packages
            .get(member)
            .expect("selected member should exist in workspace packages");
        PackageSelection {
            member: package.member.clone(),
            name: package.name.clone(),
        }
    }

    fn ordered_package_selections(&self, members: &BTreeSet<String>) -> Vec<PackageSelection> {
        self.member_order
            .iter()
            .filter(|member| members.contains(*member))
            .map(|member| self.package_selection(member))
            .collect()
    }

    fn reverse_dependency_closure(&self, direct_members: &BTreeSet<String>) -> BTreeSet<String> {
        let mut selected = direct_members.clone();
        let mut queue = direct_members.iter().cloned().collect::<VecDeque<_>>();
        while let Some(member) = queue.pop_front() {
            let Some(dependents) = self.reverse_dependencies.get(&member) else {
                continue;
            };
            for dependent in dependents {
                if selected.insert(dependent.clone()) {
                    queue.push_back(dependent.clone());
                }
            }
        }
        selected
    }
}

pub fn select_from_files(
    workspace: &Workspace,
    changed_files: &Path,
    global_fixtures: &Path,
) -> Result<Selection, Box<dyn std::error::Error>> {
    let changed_text = fs::read_to_string(changed_files)?;
    let global_fixture_text = fs::read_to_string(global_fixtures)?;
    let fixture_path = path_to_repo_string(global_fixtures);
    select_from_text(
        workspace,
        &changed_text,
        &global_fixture_text,
        &fixture_path,
    )
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e).into())
}

pub fn select_from_text(
    workspace: &Workspace,
    changed_text: &str,
    global_fixture_text: &str,
    global_fixture_path: &str,
) -> Result<Selection, String> {
    let mut global_triggers = DEFAULT_GLOBAL_TRIGGER_PATHS
        .iter()
        .map(|path| normalize_repo_path(path))
        .collect::<Result<BTreeSet<_>, _>>()?;
    global_triggers.insert(normalize_repo_path(global_fixture_path)?);
    for path in parse_fixture_paths(global_fixture_text)? {
        global_triggers.insert(path);
    }

    let mut direct_members = BTreeSet::new();
    let mut full_triggers = Vec::new();
    let mut changed_path_count = 0;

    for raw_path in changed_text.lines() {
        let raw_path = raw_path.strip_suffix('\r').unwrap_or(raw_path);
        if raw_path.is_empty() {
            continue;
        }
        changed_path_count += 1;
        let path = match normalize_repo_path(raw_path) {
            Ok(path) => path,
            Err(reason) => {
                full_triggers.push(FullTrigger {
                    path: raw_path.to_string(),
                    reason,
                });
                continue;
            }
        };
        if global_triggers.contains(&path) {
            full_triggers.push(FullTrigger {
                path,
                reason: "global trigger".to_string(),
            });
            continue;
        }
        if let Some(member) = workspace.owning_member(&path) {
            direct_members.insert(member);
        } else {
            full_triggers.push(FullTrigger {
                path,
                reason: "outside workspace crate".to_string(),
            });
        }
    }

    if changed_path_count == 0 {
        full_triggers.push(FullTrigger {
            path: "<empty>".to_string(),
            reason: "changed-files file is empty".to_string(),
        });
    }

    let direct_packages = workspace.ordered_package_selections(&direct_members);
    if !full_triggers.is_empty() {
        return Ok(Selection {
            mode: SelectionMode::Full,
            direct_packages,
            selected_packages: Vec::new(),
            full_triggers,
        });
    }

    let selected_members = workspace.reverse_dependency_closure(&direct_members);
    Ok(Selection {
        mode: SelectionMode::Scoped,
        direct_packages,
        selected_packages: workspace.ordered_package_selections(&selected_members),
        full_triggers,
    })
}

fn parse_fixture_paths(fixture_text: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for raw_line in fixture_text.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line).trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        paths.push(normalize_repo_path(line)?);
    }
    Ok(paths)
}

fn normalize_repo_path(path: &str) -> Result<String, String> {
    if path.starts_with('/') {
        return Err("absolute path is not repo-relative".to_string());
    }
    let mut path = path;
    while let Some(stripped) = path.strip_prefix("./") {
        path = stripped;
    }
    let mut components = Vec::new();
    for component in path.split('/') {
        if component.is_empty() {
            continue;
        }
        if component == "." {
            return Err("path contains a . component".to_string());
        }
        if component == ".." {
            return Err("path escapes the repository".to_string());
        }
        components.push(component);
    }
    if components.is_empty() {
        return Err("path is empty".to_string());
    }
    Ok(components.join("/"))
}

fn topological_order(
    mut vertices: Vec<String>,
    reverse_dependencies: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<String>, String> {
    let mut edges = reverse_dependencies
        .iter()
        .flat_map(|(dependency, dependents)| {
            dependents
                .iter()
                .map(|dependent| (dependency.clone(), dependent.clone()))
        })
        .collect::<Vec<_>>();
    let mut ordered = Vec::new();
    vertices.sort();
    while !vertices.is_empty() {
        let mut candidates = vertices
            .iter()
            .filter(|vertex| !edges.iter().any(|(_, dependent)| dependent == *vertex))
            .cloned()
            .collect::<Vec<_>>();
        candidates.sort();
        let Some(candidate) = candidates.first().cloned() else {
            return Err("workspace dependency graph has a cycle".to_string());
        };
        vertices.retain(|vertex| vertex != &candidate);
        edges.retain(|(dependency, _)| dependency != &candidate);
        ordered.push(candidate);
    }
    Ok(ordered)
}

fn path_to_repo_string(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
    workspace_root: String,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    id: String,
    manifest_path: String,
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        FullTrigger, PackageSelection, SelectionMode, Workspace, WorkspacePackage,
        normalize_repo_path, select_from_text,
    };

    fn workspace() -> Workspace {
        Workspace::from_parts(
            vec![
                package("a", "a"),
                package("b", "b"),
                package("c", "c"),
                package("rc_conf", "rc_conf"),
                package("rc_conf/demo_crate", "rc_conf_demo_crate"),
            ],
            vec![
                ("a".to_string(), "b".to_string()),
                ("b".to_string(), "c".to_string()),
                ("rc_conf".to_string(), "rc_conf/demo_crate".to_string()),
            ],
        )
        .unwrap()
    }

    fn package(member: &str, name: &str) -> WorkspacePackage {
        WorkspacePackage {
            member: member.to_string(),
            name: name.to_string(),
        }
    }

    fn selected(member: &str, name: &str) -> PackageSelection {
        PackageSelection {
            member: member.to_string(),
            name: name.to_string(),
        }
    }

    #[test]
    fn normalize_repo_paths_lexically() {
        assert_eq!(
            normalize_repo_path("./a//src/lib.rs").unwrap(),
            "a/src/lib.rs"
        );
        assert_eq!(
            normalize_repo_path(".ci.global-fixtures").unwrap(),
            ".ci.global-fixtures"
        );
        assert!(normalize_repo_path("/a/src/lib.rs").is_err());
        assert!(normalize_repo_path("a/../b").is_err());
        assert!(normalize_repo_path("a/./b").is_err());
    }

    #[test]
    fn selects_changed_crate_and_reverse_dependents() {
        let selection =
            select_from_text(&workspace(), "a/src/lib.rs\n", "", ".ci.global-fixtures").unwrap();
        assert_eq!(selection.mode, SelectionMode::Scoped);
        assert_eq!(selection.direct_packages, vec![selected("a", "a")]);
        assert_eq!(
            selection.selected_packages,
            vec![selected("a", "a"), selected("b", "b"), selected("c", "c")]
        );
    }

    #[test]
    fn nested_workspace_crate_uses_longest_matching_member() {
        let selection = select_from_text(
            &workspace(),
            "rc_conf/demo_crate/src/main.rs\n",
            "",
            ".ci.global-fixtures",
        )
        .unwrap();
        assert_eq!(
            selection.direct_packages,
            vec![selected("rc_conf/demo_crate", "rc_conf_demo_crate")]
        );
    }

    #[test]
    fn global_paths_force_full_ci() {
        let selection =
            select_from_text(&workspace(), "Cargo.toml\n", "", ".ci.global-fixtures").unwrap();
        assert_eq!(selection.mode, SelectionMode::Full);
        assert_eq!(
            selection.full_triggers,
            vec![FullTrigger {
                path: "Cargo.toml".to_string(),
                reason: "global trigger".to_string()
            }]
        );
    }

    #[test]
    fn fixture_paths_force_full_ci() {
        let selection = select_from_text(
            &workspace(),
            "a/tests/fixture.txt\n",
            "# global fixtures\n\na/tests/fixture.txt\n",
            ".ci.global-fixtures",
        )
        .unwrap();
        assert_eq!(selection.mode, SelectionMode::Full);
        assert_eq!(
            selection.full_triggers,
            vec![FullTrigger {
                path: "a/tests/fixture.txt".to_string(),
                reason: "global trigger".to_string()
            }]
        );
    }

    #[test]
    fn outside_workspace_paths_force_full_ci() {
        let selection =
            select_from_text(&workspace(), "README.md\n", "", ".ci.global-fixtures").unwrap();
        assert_eq!(selection.mode, SelectionMode::Full);
        assert_eq!(
            selection.full_triggers,
            vec![FullTrigger {
                path: "README.md".to_string(),
                reason: "outside workspace crate".to_string()
            }]
        );
    }

    #[test]
    fn empty_changed_files_force_full_ci() {
        let selection = select_from_text(&workspace(), "\n", "", ".ci.global-fixtures").unwrap();
        assert_eq!(selection.mode, SelectionMode::Full);
        assert_eq!(
            selection.full_triggers,
            vec![FullTrigger {
                path: "<empty>".to_string(),
                reason: "changed-files file is empty".to_string()
            }]
        );
    }
}
