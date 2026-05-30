use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use chrono::Local;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;

use ci::{candidate_order, package};

const PUBLISH_SCRIPT: &str = "publish.sh";
const USAGE: &str = "usage: publish [--prepare]";

#[derive(Debug, Deserialize)]
struct CratesIoVersionResponse {
    version: CratesIoVersion,
}

#[derive(Debug, Deserialize)]
struct CratesIoVersion {
    num: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Publish,
    Print,
    Stay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PublishCommand {
    crate_name: String,
    version: Version,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum VersionSelection {
    Current,
    New(Version),
    Skip,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    PrintPackages,
    PrepareRelease,
}

fn main() -> Result<(), Box<dyn Error>> {
    match parse_mode(env::args().skip(1))? {
        Mode::PrintPackages => print_packages(),
        Mode::PrepareRelease => prepare_release(),
    }
}

fn parse_mode<I, S>(args: I) -> Result<Mode, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [] => Ok(Mode::PrintPackages),
        [arg] if arg.as_ref() == "--prepare" => Ok(Mode::PrepareRelease),
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE).into()),
    }
}

fn print_packages() -> Result<(), Box<dyn Error>> {
    for member in candidate_order() {
        println!("{member}");
    }
    Ok(())
}

fn prepare_release() -> Result<(), Box<dyn Error>> {
    ensure_clean_worktree()?;

    let branch_name = format!("publish-{}", Local::now().format("%Y-%m-%d"));
    let client = Client::builder()
        .user_agent("https://github.com/rescrv/blue;crate:meta;bin:publish")
        .build()?;
    let mut branch_created = false;
    let mut planned_publishes = Vec::new();

    for member in candidate_order() {
        let package = package(&member);
        let crates_exists = published_on_crates_io(&client, &package.name, &package.version)?;
        let tag_exists = tag_exists(&package.name, &package.version)?;
        let has_changed = if tag_exists && crates_exists {
            has_changed_since_tag(&package.name, &package.version, &package.member)?
        } else {
            false
        };

        match classify(tag_exists, crates_exists, has_changed) {
            Action::Stay => {
                println!(
                    "{} {}: tag exists, crates.io version exists, no changes; staying",
                    package.name, package.version
                );
            }
            Action::Print => {
                if tag_exists {
                    println!(
                        "{} {}: git tag exists but crates.io version is missing; inspect manually",
                        package.name, package.version
                    );
                } else {
                    println!(
                        "{} {}: crates.io version exists without a matching tag; inspect manually",
                        package.name, package.version
                    );
                }
            }
            Action::Publish => {
                let Some(publish) = prepare_publish(
                    &package.name,
                    &package.version,
                    &package.member,
                    crates_exists,
                    tag_exists,
                    &branch_name,
                    &mut branch_created,
                )?
                else {
                    continue;
                };
                planned_publishes.push(publish);
            }
        }
    }

    if planned_publishes.is_empty() {
        println!("no publishes selected; no branch created and no publish script written");
        return Ok(());
    }

    commit_version_bumps(&branch_name)?;
    write_publish_script(&planned_publishes)?;
    println!(
        "wrote {} with {} tag commands followed by {} cargo publish commands",
        PUBLISH_SCRIPT,
        planned_publishes.len(),
        planned_publishes.len()
    );
    Ok(())
}

fn classify(tag_exists: bool, crates_exists: bool, has_changed: bool) -> Action {
    match (tag_exists, crates_exists, has_changed) {
        (true, true, true) => Action::Publish,
        (true, true, false) => Action::Stay,
        (false, true, _) => Action::Print,
        (false, false, _) => Action::Publish,
        (true, false, _) => Action::Print,
    }
}

fn published_on_crates_io(
    client: &Client,
    crate_name: &str,
    version: &Version,
) -> Result<bool, Box<dyn Error>> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}/{version}");
    let response = client.get(url).send()?;
    match response.status() {
        StatusCode::OK => {
            let response: CratesIoVersionResponse = response.json()?;
            Ok(response.version.num == version.to_string())
        }
        StatusCode::NOT_FOUND => Ok(false),
        status => {
            Err(format!("crates.io lookup failed for {crate_name} {version}: {status}").into())
        }
    }
}

fn tag_exists(crate_name: &str, version: &Version) -> Result<bool, Box<dyn Error>> {
    let tag = format!("refs/tags/{crate_name}@{version}");
    let status = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &tag])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
}

fn has_changed_since_tag(
    crate_name: &str,
    version: &Version,
    member: &str,
) -> Result<bool, Box<dyn Error>> {
    let tag = format!("refs/tags/{crate_name}@{version}");
    let status = Command::new("git")
        .args(["diff", "--quiet", &tag, "--", member])
        .status()?;
    match status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(format!("git diff failed for {crate_name}@{version}").into()),
    }
}

fn prepare_publish(
    crate_name: &str,
    current_version: &Version,
    member: &str,
    current_version_exists_on_crates_io: bool,
    current_tag_exists: bool,
    branch_name: &str,
    branch_created: &mut bool,
) -> Result<Option<PublishCommand>, Box<dyn Error>> {
    println!();
    println!("publishing candidate: {crate_name} {current_version}");
    if current_tag_exists {
        println!(
            "review delta with: git diff refs/tags/{crate_name}@{current_version} -- {member}"
        );
    } else {
        println!("review history with: git log --stat -- {member}");
    }

    let allow_current_version = !current_version_exists_on_crates_io && !current_tag_exists;
    let new_version = loop {
        let new_version =
            match prompt_for_version(crate_name, current_version, allow_current_version)? {
                VersionSelection::Current => current_version.clone(),
                VersionSelection::New(new_version) => new_version,
                VersionSelection::Skip => {
                    println!("skipping {crate_name} {current_version}");
                    return Ok(None);
                }
            };
        if tag_exists(crate_name, &new_version)? {
            println!("{crate_name} {new_version} already has a git tag");
            continue;
        }
        break new_version;
    };

    if !*branch_created {
        create_branch(branch_name)?;
        *branch_created = true;
    }

    if &new_version != current_version {
        let new_version = new_version.to_string();
        run(Command::new("cargo").args([
            "run",
            "-p",
            "ci",
            "--bin",
            "update-version",
            "--",
            "bump",
            crate_name,
            &new_version,
        ]))?;
    } else {
        println!(
            "using current unpublished version {crate_name} {new_version} without rewriting manifests"
        );
    }

    println!("buffered: git tag {crate_name}@{new_version} HEAD && cargo publish -p {crate_name}");
    Ok(Some(PublishCommand {
        crate_name: crate_name.to_string(),
        version: new_version,
    }))
}

fn create_branch(branch_name: &str) -> Result<(), Box<dyn Error>> {
    run(Command::new("git").args(["switch", "-c", branch_name]))?;
    println!("created branch {branch_name}");
    Ok(())
}

fn commit_version_bumps(branch_name: &str) -> Result<(), Box<dyn Error>> {
    run(Command::new("git").args(["add", "-u"]))?;
    if index_is_clean()? {
        println!("no version bump edits to commit on {branch_name}");
        return Ok(());
    }
    let message = format!("publish {}", Local::now().format("%Y-%m-%d"));
    run(Command::new("git").args(["commit", "-m", &message]))?;
    println!("committed version bumps on {branch_name}");
    Ok(())
}

fn write_publish_script(planned_publishes: &[PublishCommand]) -> Result<(), Box<dyn Error>> {
    let contents = render_publish_script(planned_publishes);
    fs::write(PUBLISH_SCRIPT, contents)?;
    let permissions = fs::Permissions::from_mode(0o755);
    fs::set_permissions(PUBLISH_SCRIPT, permissions)?;
    Ok(())
}

fn render_publish_script(planned_publishes: &[PublishCommand]) -> String {
    let mut script = String::from("#!/usr/bin/env bash\nset -euo pipefail\n\n");
    script.push_str("# Tag the release commit for every crate before publishing.\n");
    for publish in planned_publishes {
        script.push_str(&format!(
            "git tag {}@{} HEAD\n",
            publish.crate_name, publish.version
        ));
    }
    script.push('\n');
    script.push_str("# Publish in topological order after all tags are in place.\n");
    for publish in planned_publishes {
        script.push_str(&format!("cargo publish -p {}\n", publish.crate_name));
    }
    script
}

fn prompt_for_version(
    crate_name: &str,
    current_version: &Version,
    allow_current_version: bool,
) -> Result<VersionSelection, Box<dyn Error>> {
    if allow_current_version {
        print!(
            "what version would you like to publish for {crate_name} (current {current_version}; press enter to keep current, or type skip)? "
        );
    } else {
        print!(
            "what version would you like to publish for {crate_name} (current {current_version})? "
        );
    }
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    parse_version_selection(&line, allow_current_version)
}

fn parse_version_selection(
    line: &str,
    allow_current_version: bool,
) -> Result<VersionSelection, Box<dyn Error>> {
    let line = line.trim();
    if line.eq_ignore_ascii_case("skip") {
        return Ok(VersionSelection::Skip);
    }
    if line.is_empty() {
        return if allow_current_version {
            Ok(VersionSelection::Current)
        } else {
            Ok(VersionSelection::Skip)
        };
    }
    Ok(VersionSelection::New(line.parse()?))
}

fn ensure_clean_worktree() -> Result<(), Box<dyn Error>> {
    let output = Command::new("git").args(["status", "--short"]).output()?;
    if !output.status.success() {
        return Err("git status --short failed".into());
    }
    if !output.stdout.is_empty() {
        return Err(
            "publish preparation requires a clean worktree before creating the release branch"
                .into(),
        );
    }
    if Path::new(PUBLISH_SCRIPT).exists() {
        return Err(
            format!("{PUBLISH_SCRIPT} already exists; remove it or rename it first").into(),
        );
    }
    Ok(())
}

fn index_is_clean() -> Result<bool, Box<dyn Error>> {
    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err("git diff --cached --quiet failed".into()),
    }
}

fn run(command: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed: {command:?}").into())
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::{
        Action, Mode, PublishCommand, VersionSelection, classify, parse_mode,
        parse_version_selection, render_publish_script,
    };

    #[test]
    fn defaults_to_print_packages_mode() {
        assert_eq!(parse_mode(Vec::<&str>::new()).unwrap(), Mode::PrintPackages);
    }

    #[test]
    fn prepare_release_requires_explicit_flag() {
        assert_eq!(parse_mode(["--prepare"]).unwrap(), Mode::PrepareRelease);
    }

    #[test]
    fn rejects_unknown_args() {
        assert_eq!(
            parse_mode(["--wat"]).unwrap_err().to_string(),
            "usage: publish [--prepare]"
        );
    }

    #[test]
    fn truth_table_exists_exists_changed() {
        assert_eq!(classify(true, true, true), Action::Publish);
    }

    #[test]
    fn truth_table_exists_exists_unchanged() {
        assert_eq!(classify(true, true, false), Action::Stay);
    }

    #[test]
    fn truth_table_missing_tag_existing_crate() {
        assert_eq!(classify(false, true, false), Action::Print);
    }

    #[test]
    fn truth_table_missing_tag_missing_crate() {
        assert_eq!(classify(false, false, false), Action::Publish);
    }

    #[test]
    fn unexpected_tag_without_crate_prints() {
        assert_eq!(classify(true, false, false), Action::Print);
    }

    #[test]
    fn render_script_tags_before_publish() {
        let planned = vec![
            PublishCommand {
                crate_name: "handled".to_string(),
                version: Version::parse("0.6.0").unwrap(),
            },
            PublishCommand {
                crate_name: "lsmtk".to_string(),
                version: Version::parse("0.15.0").unwrap(),
            },
        ];
        let script = render_publish_script(&planned);
        assert!(
            script.contains("git tag handled@0.6.0 HEAD\ngit tag lsmtk@0.15.0 HEAD\n\n# Publish")
        );
        assert!(script.contains("cargo publish -p handled\ncargo publish -p lsmtk\n"));
    }

    #[test]
    fn empty_input_keeps_current_when_current_version_is_unpublished() {
        assert_eq!(
            parse_version_selection("", true).unwrap(),
            VersionSelection::Current
        );
    }

    #[test]
    fn empty_input_skips_when_new_version_is_required() {
        assert_eq!(
            parse_version_selection("", false).unwrap(),
            VersionSelection::Skip
        );
    }

    #[test]
    fn skip_input_always_skips() {
        assert_eq!(
            parse_version_selection("skip", true).unwrap(),
            VersionSelection::Skip
        );
        assert_eq!(
            parse_version_selection("skip", false).unwrap(),
            VersionSelection::Skip
        );
    }

    #[test]
    fn explicit_input_parses_a_new_version() {
        assert_eq!(
            parse_version_selection("0.15.0", true).unwrap(),
            VersionSelection::New(Version::parse("0.15.0").unwrap())
        );
    }
}
