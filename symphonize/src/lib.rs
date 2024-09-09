#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::fs::OpenOptions;

use rc_conf::RcConf;
use shvar::VariableProvider;
use utf8path::Path;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    RcConf(rc_conf::Error),
    K8sRc(k8src::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<rc_conf::Error> for Error {
    fn from(err: rc_conf::Error) -> Self {
        Self::RcConf(err)
    }
}

impl From<k8src::Error> for Error {
    fn from(err: k8src::Error) -> Self {
        Self::K8sRc(err)
    }
}

///////////////////////////////////////// SymphonizeOptions ////////////////////////////////////////

/// SymphonizeOptions provides the options to symphonize.  Provide it with a working directory and
/// release or debug mode.  Only debug mode is supported at the moment.
#[derive(Clone, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct SymphonizeOptions {
    #[arrrg(flag, "Do everything up to calling kubectl apply")]
    dry_run: bool,
}

///////////////////////////////////// auto_infer_configuration /////////////////////////////////////

pub fn paths_to_root(_options: &SymphonizeOptions) -> Result<Vec<Path<'static>>, std::io::Error> {
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

////////////////////////////////////// autoinfer_configuration /////////////////////////////////////

/// Automatically infer a Pid1Options from the SymphonizeOptions.
pub fn autoinfer_configuration(
    _options: &SymphonizeOptions,
    paths_to_root: &[Path],
) -> Result<String, Error> {
    if paths_to_root.is_empty() {
        return Err(std::io::Error::other("run symphonize within a git repository").into());
    }
    let repo = &paths_to_root[0];
    let mut rc_conf_path = k8src::rc_conf_path(paths_to_root);
    rc_conf_path += ":";
    rc_conf_path += repo.join("rc.local").as_str();
    Ok(rc_conf_path)
}

//////////////////////////////////////////// Symphonize ////////////////////////////////////////////

pub struct Symphonize {
    root: Path<'static>,
    options: SymphonizeOptions,
    rc_conf: RcConf,
}

impl Symphonize {
    pub fn new(options: SymphonizeOptions, root: Path, rc_conf: RcConf) -> Self {
        Self {
            root: root.into_owned(),
            options,
            rc_conf,
        }
    }

    pub fn apply(&mut self) -> Result<(), Error> {
        self.build_images()?;
        self.build_manifests()?;
        if !self.options.dry_run {
            self.apply_manifests()?;
        }
        Ok(())
    }

    pub fn build_images(&mut self) -> Result<(), Error> {
        for containerfile in self.rc_conf.variables() {
            let Some(prefix) = containerfile.strip_suffix("_CONTAINERFILE") else {
                continue;
            };
            let Some(containerfile) = self.rc_conf.lookup(&containerfile) else {
                continue;
            };
            let mut containerfile = Path::from(containerfile);
            if containerfile.has_app_defined() {
                containerfile = self.root.join(&containerfile.as_str()[2..]).into_owned();
            }
            let service = rc_conf::service_from_var_name(prefix);
            let Some(image) = self.rc_conf.lookup_suffix(prefix, "IMAGE") else {
                todo!();
            };
            let extra = HashMap::from_iter([
                ("IMAGE", image.clone()),
                ("CONTAINERFILE", containerfile.to_string()),
                ("CONTAINERFILE_DIRNAME", containerfile.dirname().to_string()),
            ]);
            // Build the image.
            let build = self.rc_conf.argv(&service, "IMAGE_BUILD", &extra)?;
            if build.is_empty() {
                todo!();
            }
            eprintln!("running {build:?}");
            let child = std::process::Command::new(&build[0])
                .args(&build[1..])
                .spawn()?
                .wait()?;
            if !child.success() {
                todo!();
            }
            // Push the image.
            let push = self.rc_conf.argv(&service, "IMAGE_PUSH", &extra)?;
            if push.is_empty() {
                todo!();
            }
            eprintln!("running {push:?}");
            let child = std::process::Command::new(&push[0])
                .args(&push[1..])
                .spawn()?
                .wait()?;
            if !child.success() {
                todo!();
            }
        }
        Ok(())
    }

    pub fn build_manifests(&mut self) -> Result<(), Error> {
        let options = k8src::RegenerateOptions {
            output: Some(self.target_dir().join("manifests").as_str().to_string()),
            root: Some(self.root.as_str().to_string()),
            overwrite: false,
            verify: false,
        };
        drop(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(self.target_dir().join(".k8srcignore"))?,
        );
        if self.target_dir().join("manifests").exists() {
            std::fs::remove_dir_all(self.target_dir().join("manifests"))?;
        }
        k8src::regenerate(options)?;
        Ok(())
    }

    pub fn apply_manifests(&mut self) -> Result<(), Error> {
        let cwd = Path::try_from(std::env::current_dir()?).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "current working directory not unicode",
            )
        })?;
        if !cwd.is_abs() && !cwd.has_root() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "current working directory absolute",
            )
            .into());
        }
        let Some(relative) = cwd.as_str().strip_prefix(self.root.as_str()) else {
            todo!();
        };
        let relative = relative.trim_start_matches('/');
        let output = self.target_dir().join("manifests").join(relative);
        let status = std::process::Command::new("kubectl")
            .arg("apply")
            .arg("-k")
            .arg(output.as_str())
            .spawn()?
            .wait()?;
        if status.success() {
            Ok(())
        } else {
            todo!();
        }
    }

    fn target_dir(&self) -> Path<'static> {
        if let Some(target_dir) = self.rc_conf.lookup("SYMPHONIZE_TARGET_DIR") {
            let target_dir = Path::from(target_dir);
            if target_dir.has_app_defined() {
                self.root.join(&target_dir.as_str()[2..]).into_owned()
            } else {
                target_dir
            }
        } else {
            self.root.join("target/symphonize")
        }
    }
}
