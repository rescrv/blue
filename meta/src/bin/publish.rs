use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use chrono::Local;
use reqwest::blocking::Client;
use reqwest::StatusCode;
use semver::Version;
use serde::Deserialize;

use ci::{candidate_order, package, short_version};

const PUBLISH_SCRIPT: &str = "publish.sh";

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

fn main() -> Result<(), Box<dyn Error>> {
    ensure_clean_worktree()?;

    let branch_name = format!("publish-{}", Local::now().format("%Y-%m-%d"));
    let client = Client::builder().user_agent("https://github.com/rescrv/blue;crate:meta;bin:publish").build()?;
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

    if !branch_created {
        println!("no version bumps selected; no branch created and no publish script written");
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

    let new_version = loop {
        let Some(new_version) = prompt_for_version(crate_name, current_version)? else {
            println!("skipping {crate_name} {current_version}");
            return Ok(None);
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

    run(Command::new("./update-version").args([
        crate_name,
        &current_version.to_string(),
        &short_version(current_version),
        &new_version.to_string(),
        &short_version(&new_version),
    ]))?;

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
) -> Result<Option<Version>, Box<dyn Error>> {
    print!("what version would you like to publish for {crate_name} (current {current_version})? ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    Ok(Some(line.parse()?))
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

    use super::{classify, render_publish_script, Action, PublishCommand};

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
}
