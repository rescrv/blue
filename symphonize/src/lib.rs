#![doc = include_str!("../README.md")]

use std::os::unix::process::CommandExt;

use utf8path::Path;

use rustrc::Pid1Options;

///////////////////////////////////////// SymphonizeOptions ////////////////////////////////////////

/// SymphonizeOptions provides the options to symphonize.  Provide it with a working directory and
/// release or debug mode.  Only debug mode is supported at the moment.
#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct SymphonizeOptions {
    #[arrrg(
        flag,
        "Run using cargo build --debug and the binaries in target/debug."
    )]
    pub debug: bool,
    #[arrrg(
        flag,
        "Run using release binaries and put the binaries under --workdir."
    )]
    pub release: bool,
    #[arrrg(optional, "Set the symphonize working directory.")]
    pub workdir: Option<String>,
}

////////////////////////////////////// autoinfer_configuration /////////////////////////////////////

fn paths_to_root() -> Result<Vec<Path<'static>>, std::io::Error> {
    let mut cwd = Path::try_from(std::env::current_dir()?).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "current working directory not unicode",
        )
    })?;
    if !cwd.is_abs() && !cwd.has_root() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "current working directory absolute",
        ));
    }
    let mut candidates = vec![];
    while cwd != Path::from("/") {
        candidates.push(cwd.clone().into_owned());
        if cwd.join(".git").exists() {
            candidates.reverse();
            return Ok(candidates);
        }
        cwd = cwd.dirname().into_owned();
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "no git directory found",
    ))
}

/// Automatically detect the rc_conf path given a path of candidates.
pub fn rc_conf_path(candidates: &[Path]) -> String {
    let mut rc_conf_path = String::new();
    for candidate in candidates {
        if !rc_conf_path.is_empty() {
            rc_conf_path.push(':');
        }
        rc_conf_path += candidate.join("rc.conf").as_str();
    }
    rc_conf_path
}

/// Automatically detect the rc.d path for a given root with options.
pub fn rc_d_path(options: &SymphonizeOptions, root: &Path) -> Result<String, std::io::Error> {
    let mut rc_d_paths = vec![];
    rc_d_path_recurse(options, root, &mut rc_d_paths)?;
    Ok(rc_d_paths
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>()
        .join(":"))
}

// TODO(rescrv): address this
#[allow(clippy::only_used_in_recursion)]
fn rc_d_path_recurse(
    options: &SymphonizeOptions,
    root: &Path,
    rc_d_paths: &mut Vec<Path<'static>>,
) -> Result<(), std::io::Error> {
    if root.is_dir() {
        let mut entries = vec![];
        for entry in std::fs::read_dir(root.clone().into_std())? {
            let entry = entry?;
            let Ok(path) = Path::try_from(entry.path()) else {
                continue;
            };
            entries.push(path.into_owned());
        }
        entries.sort();
        for entry in entries.into_iter() {
            if entry.as_str().ends_with("/rc.d") {
                rc_d_paths.push(entry.clone());
            }
            rc_d_path_recurse(options, &entry, rc_d_paths)?;
        }
    }
    Ok(())
}

/// Automatically infer a Pid1Options from the SymphonizeOptions.
pub fn autoinfer_configuration(
    options: &SymphonizeOptions,
) -> Result<(Pid1Options, Path<'static>), std::io::Error> {
    let paths_to_root = paths_to_root()?;
    if paths_to_root.is_empty() {
        return Err(std::io::Error::other(
            "run symphonize within a git repository",
        ));
    }
    let repo = &paths_to_root[0];
    let mut rc_conf_path = rc_conf_path(&paths_to_root);
    rc_conf_path += ":";
    rc_conf_path += repo.join("rc.local").as_str();
    let rc_d_path = rc_d_path(options, repo)?;
    Ok((
        Pid1Options {
            rc_conf_path,
            rc_d_path,
        },
        repo.clone().into_owned(),
    ))
}

////////////////////////////////////////////// rebuild /////////////////////////////////////////////

fn rebuild_deps(workdir: &Path) -> Result<(), std::io::Error> {
    // SAFETY(rescrv):  Manipulating sigprocmask is allowed between fork and exec.
    let mut child = unsafe {
        std::process::Command::new("cargo")
            .args(["vendor", workdir.join("vendor").as_str()])
            .pre_exec(|| {
                minimal_signals::unblock();
                Ok(())
            })
            .spawn()?
    };
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other("cargo vendor failed"));
    }
    // SAFETY(rescrv):  Manipulating sigprocmask is allowed between fork and exec.
    let output = unsafe {
        std::process::Command::new("cargo")
            .args(["tree", "--all-features", "--prefix", "none", "--quiet"])
            .pre_exec(|| {
                minimal_signals::unblock();
                Ok(())
            })
            .output()?
    };
    for dep in String::from_utf8_lossy(&output.stdout).split_terminator('\n') {
        // TODO(rescrv): Hacky as all get-out.
        if dep.contains("(/") {
            continue;
        }
        let Some(dep) = dep.split(' ').next() else {
            continue;
        };
        // SAFETY(rescrv):  Manipulating sigprocmask is allowed between fork and exec.
        let output = unsafe {
            std::process::Command::new("cargo")
                .args([
                    "install",
                    dep,
                    "--force",
                    "--root",
                    workdir.join(".symphonize/pkg").as_str(),
                ])
                .pre_exec(|| {
                    minimal_signals::unblock();
                    Ok(())
                })
                .output()?
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() && !stderr.contains("there is nothing to install in") {
            return Err(std::io::Error::other(format!(
                "cargo install failed:\n{stderr}"
            )));
        }
    }
    Ok(())
}

/// Rebuild the .symphonize directory in workdir.
pub fn rebuild_cargo(workdir: &Path<'static>) -> Result<(), std::io::Error> {
    rebuild_deps(workdir)?;
    // SAFETY(rescrv):  Manipulating sigprocmask is allowed between fork and exec.
    let mut child = unsafe {
        std::process::Command::new("cargo")
            .args(["build", "--workspace", "--bins"])
            .pre_exec(|| {
                minimal_signals::unblock();
                Ok(())
            })
            .spawn()?
    };
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other("cargo build failed"));
    }
    Ok(())
}

/// TODO(rescrv): implement.
pub fn rebuild_release(_workdir: &Path<'static>) -> Result<(), std::io::Error> {
    todo!("release mode not yet supported");
}
