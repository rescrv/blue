use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use semver::Version;
use toml::{Table, Value};

const USAGE: &str =
    "usage: update-version (bump <crate> <new-version>|plan <crate> <new-version>|normalize)";

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Mode {
    Bump {
        crate_name: String,
        new_version: Version,
    },
    Plan {
        crate_name: String,
        new_version: Version,
    },
    Normalize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Package {
    member: String,
    name: String,
    version: Version,
    manifest_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Workspace {
    packages: BTreeMap<String, Package>,
    member_to_name: BTreeMap<String, String>,
    local_names: BTreeSet<String>,
    root_workspace_dependencies: BTreeSet<String>,
    reverse_dependencies: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VersionChange {
    old: Version,
    new: Version,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizationEdit {
    path: PathBuf,
    line: usize,
    old: String,
    new: String,
}

fn main() -> Result<()> {
    match parse_mode(env::args().skip(1))? {
        Mode::Bump {
            crate_name,
            new_version,
        } => {
            let workspace = read_workspace()?;
            let changes = planned_bumps(&workspace, &crate_name, &new_version)?;
            let report = apply_manifest_changes(&workspace, &changes, true)?;
            print_apply_report(&report);
            run_cargo_check()
        }
        Mode::Plan {
            crate_name,
            new_version,
        } => {
            let workspace = read_workspace()?;
            let changes = planned_bumps(&workspace, &crate_name, &new_version)?;
            let normalizations = planned_normalizations(&workspace)?;
            print_plan(&workspace, &changes, &normalizations);
            Ok(())
        }
        Mode::Normalize => {
            let workspace = read_workspace()?;
            let report = apply_manifest_changes(&workspace, &BTreeMap::new(), true)?;
            print_apply_report(&report);
            run_cargo_check()
        }
    }
}

fn parse_mode<I, S>(args: I) -> Result<Mode>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect::<Vec<_>>();
    match args.as_slice() {
        [mode, crate_name, version] if mode == "bump" => Ok(Mode::Bump {
            crate_name: crate_name.to_string(),
            new_version: version.parse()?,
        }),
        [mode, crate_name, version] if mode == "plan" => Ok(Mode::Plan {
            crate_name: crate_name.to_string(),
            new_version: version.parse()?,
        }),
        [mode] if mode == "normalize" => Ok(Mode::Normalize),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE).into()),
    }
}

fn read_workspace() -> Result<Workspace> {
    let root_text = fs::read_to_string("Cargo.toml")?;
    let root_toml = root_text.parse::<Table>()?;
    let workspace_table = root_toml
        .get("workspace")
        .and_then(Value::as_table)
        .ok_or("root Cargo.toml is missing [workspace]")?;
    let members = workspace_table
        .get("members")
        .and_then(Value::as_array)
        .ok_or("root Cargo.toml is missing workspace.members")?;
    let root_workspace_dependencies = workspace_table
        .get("dependencies")
        .and_then(Value::as_table)
        .map(|deps| deps.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();

    let mut packages = BTreeMap::new();
    let mut member_to_name = BTreeMap::new();
    for member in members {
        let member = member
            .as_str()
            .ok_or("workspace.members contains a non-string member")?;
        let manifest_path = PathBuf::from(member).join("Cargo.toml");
        let manifest_text = fs::read_to_string(&manifest_path)?;
        let manifest = manifest_text.parse::<Table>()?;
        let package_table = manifest
            .get("package")
            .and_then(Value::as_table)
            .ok_or_else(|| format!("{} is missing [package]", manifest_path.display()))?;
        let name = package_table
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} is missing package.name", manifest_path.display()))?
            .to_string();
        let version = package_table
            .get("version")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} is missing package.version", manifest_path.display()))?
            .parse()?;
        if packages.contains_key(&name) {
            return Err(format!("duplicate workspace package name: {name}").into());
        }
        member_to_name.insert(member.to_string(), name.clone());
        packages.insert(
            name.clone(),
            Package {
                member: member.to_string(),
                name,
                version,
                manifest_path,
            },
        );
    }

    let local_names = packages.keys().cloned().collect::<BTreeSet<_>>();
    let mut reverse_dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for package in packages.values() {
        reverse_dependencies
            .entry(package.name.clone())
            .or_default();
        let manifest_text = fs::read_to_string(&package.manifest_path)?;
        let manifest = manifest_text.parse::<Table>()?;
        for dependency in dependency_names(&manifest) {
            if local_names.contains(&dependency) && dependency != package.name {
                reverse_dependencies
                    .entry(dependency)
                    .or_default()
                    .insert(package.name.clone());
            }
        }
    }

    Ok(Workspace {
        packages,
        member_to_name,
        local_names,
        root_workspace_dependencies,
        reverse_dependencies,
    })
}

fn dependency_names(manifest: &Table) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    collect_dependencies(manifest, &mut deps);
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for target in targets.values() {
            if let Some(target) = target.as_table() {
                collect_dependencies(target, &mut deps);
            }
        }
    }
    deps
}

fn collect_dependencies(manifest: &Table, deps: &mut BTreeSet<String>) {
    for heading in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(dependencies) = manifest.get(heading).and_then(Value::as_table) else {
            continue;
        };
        for (dependency, spec) in dependencies {
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

fn planned_bumps(
    workspace: &Workspace,
    crate_name: &str,
    new_version: &Version,
) -> Result<BTreeMap<String, VersionChange>> {
    let package = resolve_package(workspace, crate_name)?;
    if new_version <= &package.version {
        return Err(format!(
            "{} is already {}; requested version must be greater than the current version",
            package.name, package.version
        )
        .into());
    }

    let mut changes = BTreeMap::new();
    changes.insert(
        package.name.clone(),
        VersionChange {
            old: package.version.clone(),
            new: new_version.clone(),
        },
    );
    for dependent in reverse_dependency_closure(&package.name, &workspace.reverse_dependencies) {
        let package = workspace
            .packages
            .get(&dependent)
            .expect("reverse dependency closure references known packages");
        changes.insert(
            dependent,
            VersionChange {
                old: package.version.clone(),
                new: automatic_bump(&package.version),
            },
        );
    }
    Ok(changes)
}

fn resolve_package<'a>(workspace: &'a Workspace, crate_name: &str) -> Result<&'a Package> {
    if let Some(package) = workspace.packages.get(crate_name) {
        return Ok(package);
    }
    if let Some(name) = workspace.member_to_name.get(crate_name) {
        return Ok(workspace
            .packages
            .get(name)
            .expect("member_to_name references known packages"));
    }
    Err(format!("unknown workspace crate: {crate_name}").into())
}

fn reverse_dependency_closure(
    start: &str,
    reverse_dependencies: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_string()]);
    while let Some(package) = queue.pop_front() {
        let Some(dependents) = reverse_dependencies.get(&package) else {
            continue;
        };
        for dependent in dependents {
            if dependent != start && seen.insert(dependent.clone()) {
                queue.push_back(dependent.clone());
            }
        }
    }
    seen
}

fn automatic_bump(version: &Version) -> Version {
    if version.major == 0 && version.patch == 0 {
        Version::new(0, version.minor + 1, 0)
    } else {
        Version::new(version.major + 1, 0, 0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ApplyReport {
    version_updates: usize,
    workspace_dependency_updates: usize,
    normalization_edits: Vec<NormalizationEdit>,
}

fn apply_manifest_changes(
    workspace: &Workspace,
    changes: &BTreeMap<String, VersionChange>,
    normalize: bool,
) -> Result<ApplyReport> {
    let mut version_updates = 0;
    let mut normalization_edits = Vec::new();
    let mut writes = Vec::new();

    for package in workspace.packages.values() {
        let manifest_text = fs::read_to_string(&package.manifest_path)?;
        let new_version = changes.get(&package.name).map(|change| &change.new);
        let (new_manifest_text, edits, version_updated) = rewrite_member_manifest(
            &manifest_text,
            package,
            new_version,
            normalize.then_some(&workspace.local_names),
        )?;
        if new_version.is_some() && !version_updated {
            return Err(format!(
                "failed to update package.version in {}",
                package.manifest_path.display()
            )
            .into());
        }
        if version_updated {
            version_updates += 1;
        }
        normalization_edits.extend(edits);
        if new_manifest_text != manifest_text {
            writes.push((package.manifest_path.clone(), new_manifest_text));
        }
    }

    let workspace_dependency_updates = if changes.is_empty() {
        0
    } else {
        let root_path = PathBuf::from("Cargo.toml");
        let root_text = fs::read_to_string(&root_path)?;
        let (new_root_text, updates) =
            rewrite_root_workspace_dependencies(&root_text, workspace, changes)?;
        if new_root_text != root_text {
            writes.push((root_path, new_root_text));
        }
        updates
    };

    for (path, contents) in writes {
        fs::write(path, contents)?;
    }

    Ok(ApplyReport {
        version_updates,
        workspace_dependency_updates,
        normalization_edits,
    })
}

fn rewrite_member_manifest(
    manifest_text: &str,
    package: &Package,
    new_version: Option<&Version>,
    local_names: Option<&BTreeSet<String>>,
) -> Result<(String, Vec<NormalizationEdit>, bool)> {
    let mut section = String::new();
    let mut edits = Vec::new();
    let mut version_updated = false;
    let rewritten = rewrite_text_lines(manifest_text, |line_number, line| {
        if let Some(next_section) = section_name(line) {
            section = next_section;
            return Ok(None);
        }

        if section == "package"
            && let Some(version) = new_version
            && let Some(replacement) =
                rewrite_package_version_line(line, &package.version, version)?
        {
            version_updated = true;
            return Ok(Some(replacement));
        }

        if is_dependency_section(&section)
            && let Some(local_names) = local_names
            && let Some(replacement) = normalize_dependency_line(line, local_names)?
        {
            edits.push(NormalizationEdit {
                path: package.manifest_path.clone(),
                line: line_number,
                old: line.to_string(),
                new: replacement.clone(),
            });
            return Ok(Some(replacement));
        }

        Ok(None)
    })?;
    Ok((rewritten, edits, version_updated))
}

fn rewrite_package_version_line(
    line: &str,
    old_version: &Version,
    new_version: &Version,
) -> Result<Option<String>> {
    let Some((indent, key, rhs)) = split_assignment(line) else {
        return Ok(None);
    };
    if unquote_key(key) != "version" {
        return Ok(None);
    }
    let value = parse_assignment_value(rhs)?;
    if value.as_str() != Some(&old_version.to_string()) {
        return Ok(None);
    }
    Ok(Some(format!("{indent}version = \"{new_version}\"")))
}

fn normalize_dependency_line(line: &str, local_names: &BTreeSet<String>) -> Result<Option<String>> {
    let Some((indent, key, rhs)) = split_assignment(line) else {
        return Ok(None);
    };
    let value = match parse_assignment_value(rhs) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let dependency = dependency_name(&unquote_key(key), &value);
    if !local_names.contains(&dependency) || is_normalized_workspace_spec(&value) {
        return Ok(None);
    }
    Ok(Some(format!(
        "{indent}{key} = {}",
        render_workspace_dependency_spec(&value)
    )))
}

fn is_normalized_workspace_spec(value: &Value) -> bool {
    let Some(table) = value.as_table() else {
        return false;
    };
    table.get("workspace").and_then(Value::as_bool) == Some(true)
        && !table.contains_key("path")
        && !table.contains_key("version")
}

fn render_workspace_dependency_spec(value: &Value) -> String {
    let mut fields = vec!["workspace = true".to_string()];
    if let Some(table) = value.as_table() {
        for key in ["package", "optional", "features", "default-features"] {
            if let Some(value) = table.get(key) {
                fields.push(format!("{} = {}", render_key(key), value));
            }
        }
        for (key, value) in table {
            if matches!(
                key.as_str(),
                "workspace"
                    | "path"
                    | "version"
                    | "package"
                    | "optional"
                    | "features"
                    | "default-features"
            ) {
                continue;
            }
            fields.push(format!("{} = {}", render_key(key), value));
        }
    }
    format!("{{ {} }}", fields.join(", "))
}

fn render_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        key.to_string()
    } else {
        format!("\"{}\"", key.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn rewrite_root_workspace_dependencies(
    manifest_text: &str,
    workspace: &Workspace,
    changes: &BTreeMap<String, VersionChange>,
) -> Result<(String, usize)> {
    let mut updated = BTreeSet::new();
    let mut section = String::new();
    let rewritten = rewrite_text_lines(manifest_text, |_, line| {
        if let Some(next_section) = section_name(line) {
            section = next_section;
            return Ok(None);
        }
        if section != "workspace.dependencies" {
            return Ok(None);
        }
        let Some((_, key, rhs)) = split_assignment(line) else {
            return Ok(None);
        };
        let key = unquote_key(key);
        let Some(change) = changes.get(&key) else {
            return Ok(None);
        };
        let replacement = rewrite_dependency_version_line(line, rhs, &change.old, &change.new)?;
        if replacement.is_some() {
            updated.insert(key);
        }
        Ok(replacement)
    })?;

    for package in changes.keys() {
        if workspace.root_workspace_dependencies.contains(package) && !updated.contains(package) {
            return Err(
                format!("failed to update workspace dependency version for {package}").into(),
            );
        }
    }

    let updates = updated.len();
    Ok((rewritten, updates))
}

fn rewrite_dependency_version_line(
    line: &str,
    rhs: &str,
    old_version: &Version,
    new_version: &Version,
) -> Result<Option<String>> {
    let old_literal = format!("version = \"{old_version}\"");
    let new_literal = format!("version = \"{new_version}\"");
    if line.contains(&old_literal) {
        return Ok(Some(line.replacen(&old_literal, &new_literal, 1)));
    }

    let value = parse_assignment_value(rhs)?;
    let Some(actual_version) = value
        .as_table()
        .and_then(|table| table.get("version"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    let actual_literal = format!("version = \"{actual_version}\"");
    if line.contains(&actual_literal) {
        return Ok(Some(line.replacen(&actual_literal, &new_literal, 1)));
    }
    Ok(None)
}

fn planned_normalizations(workspace: &Workspace) -> Result<Vec<NormalizationEdit>> {
    let mut edits = Vec::new();
    for package in workspace.packages.values() {
        let manifest_text = fs::read_to_string(&package.manifest_path)?;
        let (_, package_edits, _) =
            rewrite_member_manifest(&manifest_text, package, None, Some(&workspace.local_names))?;
        edits.extend(package_edits);
    }
    Ok(edits)
}

fn rewrite_text_lines<F>(text: &str, mut rewrite: F) -> Result<String>
where
    F: FnMut(usize, &str) -> Result<Option<String>>,
{
    let mut rewritten = String::with_capacity(text.len());
    for (index, chunk) in text.split_inclusive('\n').enumerate() {
        let (line, newline) = chunk
            .strip_suffix('\n')
            .map(|line| (line, "\n"))
            .unwrap_or((chunk, ""));
        let (line, carriage_return) = line
            .strip_suffix('\r')
            .map(|line| (line, "\r"))
            .unwrap_or((line, ""));
        if let Some(replacement) = rewrite(index + 1, line)? {
            rewritten.push_str(&replacement);
        } else {
            rewritten.push_str(line);
        }
        rewritten.push_str(carriage_return);
        rewritten.push_str(newline);
    }
    Ok(rewritten)
}

fn section_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let section = trimmed.trim_start_matches('[').trim_end_matches(']');
    (!section.is_empty()).then(|| section.to_string())
}

fn is_dependency_section(section: &str) -> bool {
    matches!(
        section,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    ) || section.ends_with(".dependencies")
        || section.ends_with(".dev-dependencies")
        || section.ends_with(".build-dependencies")
}

fn split_assignment(line: &str) -> Option<(&str, &str, &str)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let indent_len = line.len() - trimmed.len();
    let equals = trimmed.find('=')?;
    let indent = &line[..indent_len];
    let key = trimmed[..equals].trim();
    let rhs = trimmed[equals + 1..].trim();
    if key.is_empty() || rhs.is_empty() {
        return None;
    }
    Some((indent, key, rhs))
}

fn parse_assignment_value(rhs: &str) -> Result<Value> {
    let mut table = format!("value = {rhs}\n").parse::<Table>()?;
    table
        .remove("value")
        .ok_or_else(|| "assignment parser did not return a value".into())
}

fn unquote_key(key: &str) -> String {
    let key = key.trim();
    if key.len() >= 2 && key.starts_with('"') && key.ends_with('"') {
        key[1..key.len() - 1].to_string()
    } else {
        key.to_string()
    }
}

fn print_plan(
    workspace: &Workspace,
    changes: &BTreeMap<String, VersionChange>,
    normalizations: &[NormalizationEdit],
) {
    println!("version bumps:");
    for (package, change) in changes {
        let workspace_dep = if workspace.root_workspace_dependencies.contains(package) {
            " and workspace dependency entry"
        } else {
            ""
        };
        println!(
            "  {package}: {} -> {}{}",
            change.old, change.new, workspace_dep
        );
    }

    if normalizations.is_empty() {
        println!("dependency normalizations: none");
    } else {
        println!("dependency normalizations:");
        for edit in normalizations {
            println!(
                "  {}:{}: {} -> {}",
                edit.path.display(),
                edit.line,
                edit.old.trim(),
                edit.new.trim()
            );
        }
    }
}

fn print_apply_report(report: &ApplyReport) {
    println!(
        "updated {} package versions and {} workspace dependency versions",
        report.version_updates, report.workspace_dependency_updates
    );
    if report.normalization_edits.is_empty() {
        println!("normalized 0 dependency specs");
    } else {
        println!(
            "normalized {} dependency specs:",
            report.normalization_edits.len()
        );
        for edit in &report.normalization_edits {
            println!(
                "  {}:{}: {} -> {}",
                edit.path.display(),
                edit.line,
                edit.old.trim(),
                edit.new.trim()
            );
        }
    }
}

fn run_cargo_check() -> Result<()> {
    let status = Command::new("cargo")
        .args(["check", "--all-targets"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err("cargo check --all-targets failed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(version: &str) -> Version {
        version.parse().unwrap()
    }

    #[test]
    fn parses_modes() {
        assert_eq!(
            parse_mode(["bump", "handled", "0.8.0"]).unwrap(),
            Mode::Bump {
                crate_name: "handled".to_string(),
                new_version: version("0.8.0")
            }
        );
        assert_eq!(
            parse_mode(["plan", "handled", "0.8.0"]).unwrap(),
            Mode::Plan {
                crate_name: "handled".to_string(),
                new_version: version("0.8.0")
            }
        );
        assert_eq!(parse_mode(["normalize"]).unwrap(), Mode::Normalize);
        assert_eq!(parse_mode(["wat"]).unwrap_err().to_string(), USAGE);
    }

    #[test]
    fn automatic_bump_follows_repo_policy() {
        assert_eq!(automatic_bump(&version("0.7.0")), version("0.8.0"));
        assert_eq!(automatic_bump(&version("0.7.1")), version("1.0.0"));
        assert_eq!(automatic_bump(&version("1.2.3")), version("2.0.0"));
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
            BTreeSet::from([
                "arrrg".to_string(),
                "buffertk".to_string(),
                "handled".to_string(),
                "prototk".to_string(),
                "sync42".to_string(),
                "zerror".to_string(),
            ])
        );
    }

    #[test]
    fn reverse_closure_is_transitive() {
        let reverse_dependencies = BTreeMap::from([
            ("a".to_string(), BTreeSet::from(["b".to_string()])),
            ("b".to_string(), BTreeSet::from(["c".to_string()])),
            ("c".to_string(), BTreeSet::new()),
        ]);
        assert_eq!(
            reverse_dependency_closure("a", &reverse_dependencies),
            BTreeSet::from(["b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn planned_bumps_include_transitive_dependents() {
        let workspace = Workspace {
            packages: BTreeMap::from([
                (
                    "a".to_string(),
                    Package {
                        member: "a".to_string(),
                        name: "a".to_string(),
                        version: version("0.1.0"),
                        manifest_path: PathBuf::from("a/Cargo.toml"),
                    },
                ),
                (
                    "b".to_string(),
                    Package {
                        member: "b".to_string(),
                        name: "b".to_string(),
                        version: version("0.2.0"),
                        manifest_path: PathBuf::from("b/Cargo.toml"),
                    },
                ),
                (
                    "c".to_string(),
                    Package {
                        member: "c".to_string(),
                        name: "c".to_string(),
                        version: version("1.2.3"),
                        manifest_path: PathBuf::from("c/Cargo.toml"),
                    },
                ),
            ]),
            member_to_name: BTreeMap::from([
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
            ]),
            local_names: BTreeSet::from(["a".to_string(), "b".to_string(), "c".to_string()]),
            root_workspace_dependencies: BTreeSet::new(),
            reverse_dependencies: BTreeMap::from([
                ("a".to_string(), BTreeSet::from(["b".to_string()])),
                ("b".to_string(), BTreeSet::from(["c".to_string()])),
                ("c".to_string(), BTreeSet::new()),
            ]),
        };
        assert_eq!(
            planned_bumps(&workspace, "a", &version("0.3.0")).unwrap(),
            BTreeMap::from([
                (
                    "a".to_string(),
                    VersionChange {
                        old: version("0.1.0"),
                        new: version("0.3.0")
                    }
                ),
                (
                    "b".to_string(),
                    VersionChange {
                        old: version("0.2.0"),
                        new: version("0.3.0")
                    }
                ),
                (
                    "c".to_string(),
                    VersionChange {
                        old: version("1.2.3"),
                        new: version("2.0.0")
                    }
                ),
            ])
        );
    }

    #[test]
    fn normalizes_local_path_dependencies() {
        let package = Package {
            member: "caternary".to_string(),
            name: "caternary".to_string(),
            version: version("0.2.0"),
            manifest_path: PathBuf::from("caternary/Cargo.toml"),
        };
        let local_names = BTreeSet::from(["handled".to_string(), "shvar".to_string()]);
        let manifest = r#"[package]
name = "caternary"
version = "0.2.0"

[dependencies]
handled = { path = "../handled", optional = true, features = ["test"], default-features = false }
renamed = { package = "shvar", path = "../shvar" }
external = { path = "../external" }
"#;
        let (rewritten, edits, version_updated) =
            rewrite_member_manifest(manifest, &package, None, Some(&local_names)).unwrap();
        assert!(!version_updated);
        assert_eq!(edits.len(), 2);
        assert!(rewritten.contains(
            "handled = { workspace = true, optional = true, features = [\"test\"], default-features = false }"
        ));
        assert!(rewritten.contains("renamed = { workspace = true, package = \"shvar\" }"));
        assert!(rewritten.contains("external = { path = \"../external\" }"));
    }

    #[test]
    fn rewrites_package_version_without_touching_dependency_versions() {
        let package = Package {
            member: "handled".to_string(),
            name: "handled".to_string(),
            version: version("0.7.0"),
            manifest_path: PathBuf::from("handled/Cargo.toml"),
        };
        let manifest = r#"[package]
name = "handled"
version = "0.7.0"

[dependencies]
other = { version = "0.7.0" }
"#;
        let (rewritten, _, version_updated) =
            rewrite_member_manifest(manifest, &package, Some(&version("0.8.0")), None).unwrap();
        assert!(version_updated);
        assert!(rewritten.contains("version = \"0.8.0\""));
        assert!(rewritten.contains("other = { version = \"0.7.0\" }"));
    }
}
