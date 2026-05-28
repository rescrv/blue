#![doc = include_str!("../README.md")]

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::process::{Command, ExitStatus};

use rc_conf::RcConf;
use shvar::VariableProvider;
use utf8path::Path;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    RcConf(rc_conf::Error),
    K8sRc(k8src::Error),
    MissingImage {
        service: String,
    },
    MissingCommand {
        service: String,
        variable: String,
    },
    CommandIo {
        service: String,
        variable: String,
        argv: Vec<String>,
        err: std::io::Error,
    },
    CommandFailed {
        service: String,
        variable: String,
        argv: Vec<String>,
        status: ExitStatus,
    },
    CurrentDirectoryNotInRepository {
        cwd: Path<'static>,
        root: Path<'static>,
    },
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

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(err) => write!(f, "IO error: {err}"),
            Error::RcConf(err) => write!(f, "rc.conf error: {err}"),
            Error::K8sRc(k8src::Error::MissingImage { service }) => {
                write!(f, "missing IMAGE configuration for service {service}")
            }
            Error::K8sRc(err) => write!(f, "k8src error: {err:?}"),
            Error::MissingImage { service } => {
                write!(f, "missing IMAGE configuration for service {service}")
            }
            Error::MissingCommand { service, variable } => {
                write!(f, "missing {variable} command for service {service}")
            }
            Error::CommandIo {
                service,
                variable,
                argv,
                err,
            } => write!(
                f,
                "{variable} command for service {service} could not execute {:?}: {err}",
                argv
            ),
            Error::CommandFailed {
                service,
                variable,
                argv,
                status,
            } => write!(
                f,
                "{variable} command for service {service} failed with status {status}: {:?}",
                argv
            ),
            Error::CurrentDirectoryNotInRepository { cwd, root } => write!(
                f,
                "current directory {} is not inside repository root {}",
                cwd.as_str(),
                root.as_str()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::RcConf(err) => Some(err),
            Error::CommandIo { err, .. } => Some(err),
            _ => None,
        }
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
    let mut cwd = Path::try_from(std::env::current_dir()?)
        .map_err(|_| std::io::Error::other("current working directory not unicode"))?;
    if !cwd.is_abs() {
        return Err(std::io::Error::other(
            "current working directory is not absolute",
        ));
    }
    let mut candidates = vec![];
    while cwd != Path::from("/") {
        candidates.push(cwd.clone().into_owned());
        if cwd.join(".git").exists()? {
            candidates.reverse();
            return Ok(candidates);
        }
        cwd = cwd.dirname().into_owned();
    }
    Err(std::io::Error::other("no git directory found"))
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
        let mut variables = self.rc_conf.variables();
        variables.sort();
        for containerfile in variables {
            let Some(prefix) = containerfile.strip_suffix("_CONTAINERFILE") else {
                continue;
            };
            let Some(containerfile) = self.rc_conf.lookup(&containerfile) else {
                continue;
            };
            let service = rc_conf::service_from_var_name(prefix);
            let containerfile = self.resolve_app_defined_path(Path::from(containerfile));
            let Some(image) = self.rc_conf.lookup_suffix(&service, "IMAGE") else {
                return Err(Error::MissingImage { service });
            };
            let extra = HashMap::from_iter([
                ("IMAGE", image.clone()),
                ("CONTAINERFILE", containerfile.to_string()),
                ("CONTAINERFILE_DIRNAME", containerfile.dirname().to_string()),
            ]);
            let build = self.rc_conf.argv(&service, "IMAGE_BUILD", &extra)?;
            self.run_configured_command(&service, "IMAGE_BUILD", build)?;
            let push = self.rc_conf.argv(&service, "IMAGE_PUSH", &extra)?;
            self.run_configured_command(&service, "IMAGE_PUSH", push)?;
        }
        Ok(())
    }

    pub fn build_manifests(&mut self) -> Result<(), Error> {
        let target_dir = self.target_dir();
        let manifests = target_dir.join("manifests");
        let options = k8src::RegenerateOptions {
            output: Some(manifests.as_str().to_string()),
            root: Some(self.root.as_str().to_string()),
            overwrite: false,
            verify: false,
            dry_run: false,
            diff: false,
        };
        std::fs::create_dir_all(&target_dir)?;
        drop(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(target_dir.join(".k8srcignore"))?,
        );
        if manifests.exists()? {
            std::fs::remove_dir_all(manifests)?;
        }
        k8src::regenerate(options)?;
        Ok(())
    }

    pub fn apply_manifests(&mut self) -> Result<(), Error> {
        let cwd_std = std::env::current_dir()?;
        let output = self.manifests_dir_for_cwd(&cwd_std)?;
        let argv = vec![
            "kubectl".to_string(),
            "apply".to_string(),
            "-k".to_string(),
            output.as_str().to_string(),
        ];
        let status = Command::new(&argv[0])
            .arg("apply")
            .arg("-k")
            .arg(output.as_str())
            .status()
            .map_err(|err| Error::CommandIo {
                service: "manifests".to_string(),
                variable: "kubectl apply".to_string(),
                argv: argv.clone(),
                err,
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed {
                service: "manifests".to_string(),
                variable: "kubectl apply".to_string(),
                argv,
                status,
            })
        }
    }

    fn manifests_dir_for_cwd(&self, cwd_std: &std::path::Path) -> Result<Path<'static>, Error> {
        let cwd = Path::try_from(cwd_std)
            .map_err(|_| std::io::Error::other("current working directory not unicode"))?;
        if !cwd.is_abs() {
            return Err(std::io::Error::other("current working directory is not absolute").into());
        }
        let relative = cwd_std.strip_prefix(self.root.as_str()).map_err(|_| {
            Error::CurrentDirectoryNotInRepository {
                cwd: cwd.clone().into_owned(),
                root: self.root.clone(),
            }
        })?;
        let relative = Path::try_from(relative)
            .map_err(|_| std::io::Error::other("current working directory not unicode"))?;
        let mut output = self.target_dir().join("manifests");
        if !relative.as_str().is_empty() {
            output = output.join(relative).into_owned();
        };
        Ok(output)
    }

    fn target_dir(&self) -> Path<'static> {
        if let Some(target_dir) = self.rc_conf.lookup("SYMPHONIZE_TARGET_DIR") {
            self.resolve_app_defined_path(Path::from(target_dir))
        } else {
            self.root.join("target/symphonize")
        }
    }

    fn resolve_app_defined_path(&self, path: Path) -> Path<'static> {
        if path.has_app_defined() {
            self.root.join(&path.as_str()[2..]).into_owned()
        } else {
            path.into_owned()
        }
    }

    fn run_configured_command(
        &self,
        service: &str,
        variable: &str,
        argv: Vec<String>,
    ) -> Result<(), Error> {
        if argv.is_empty() {
            return Err(Error::MissingCommand {
                service: service.to_string(),
                variable: variable.to_string(),
            });
        }
        eprintln!("running {argv:?}");
        let status = Command::new(&argv[0])
            .args(&argv[1..])
            .status()
            .map_err(|err| Error::CommandIo {
                service: service.to_string(),
                variable: variable.to_string(),
                argv: argv.clone(),
                err,
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed {
                service: service.to_string(),
                variable: variable.to_string(),
                argv,
                status,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEMP_REPO_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempRepo {
        root: Path<'static>,
    }

    impl TempRepo {
        fn new() -> Self {
            let id = TEMP_REPO_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("symphonize_test_{}_{}", std::process::id(), id));
            std::fs::create_dir_all(path.join(".git")).expect("temp git dir should be writable");
            let root = Path::try_from(path).expect("temp path should be UTF-8");
            Self { root }
        }

        fn write_rc_conf(&self, contents: &str) {
            std::fs::write(self.root.join("rc.conf"), contents)
                .expect("rc.conf should be writable");
        }

        fn rc_conf(&self) -> RcConf {
            RcConf::parse(self.root.join("rc.conf").as_str()).expect("rc.conf should parse")
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn symphonize_for(repo: &TempRepo, contents: &str) -> Symphonize {
        repo.write_rc_conf(contents);
        Symphonize::new(
            SymphonizeOptions::default(),
            repo.root.clone(),
            repo.rc_conf(),
        )
    }

    #[test]
    fn build_images_requires_image_for_containerfile() {
        let repo = TempRepo::new();
        let mut symphonize = symphonize_for(
            &repo,
            r#"
svc_ENABLED="YES"
svc_CONTAINERFILE="//Containerfile"
IMAGE_BUILD=""
IMAGE_PUSH=""
"#,
        );
        match symphonize.build_images() {
            Err(Error::MissingImage { service }) => assert_eq!("svc", service),
            Err(err) => panic!("expected MissingImage; got {err:?}"),
            Ok(()) => panic!("expected MissingImage; got Ok"),
        }
    }

    #[test]
    fn build_images_requires_build_command() {
        let repo = TempRepo::new();
        let mut symphonize = symphonize_for(
            &repo,
            r#"
svc_ENABLED="YES"
svc_IMAGE="localhost/svc:latest"
svc_CONTAINERFILE="//Containerfile"
IMAGE_BUILD=""
IMAGE_PUSH=""
"#,
        );
        match symphonize.build_images() {
            Err(Error::MissingCommand { service, variable }) => {
                assert_eq!("svc", service);
                assert_eq!("IMAGE_BUILD", variable);
            }
            Err(err) => panic!("expected MissingCommand; got {err:?}"),
            Ok(()) => panic!("expected MissingCommand; got Ok"),
        }
    }

    #[test]
    fn build_manifests_creates_clean_target_dir() {
        let repo = TempRepo::new();
        repo.write_rc_conf(
            r#"
NAMESPACE="symphonize-test"
SYMPHONIZE_TARGET_DIR="//target/symphonize"
IMAGE_RCVAR=""
svc_ENABLED="YES"
svc_IMAGE="localhost/svc:latest"
svc_PORT="1234"
"#,
        );
        let mut symphonize = Symphonize::new(
            SymphonizeOptions::default(),
            repo.root.clone(),
            repo.rc_conf(),
        );
        symphonize
            .build_manifests()
            .expect("manifest generation should not need a cluster");
        assert!(
            repo.root
                .join("target/symphonize/.k8srcignore")
                .exists()
                .expect(".k8srcignore existence check should succeed")
        );
        assert!(
            repo.root
                .join("target/symphonize/manifests/herd/svc.yaml")
                .exists()
                .expect("service manifest existence check should succeed")
        );
        assert!(
            repo.root
                .join("target/symphonize/manifests/kustomization.yaml")
                .exists()
                .expect("kustomization existence check should succeed")
        );
    }

    #[test]
    fn manifests_dir_rejects_sibling_with_same_prefix() {
        let repo = TempRepo::new();
        let symphonize = symphonize_for(
            &repo,
            r#"
SYMPHONIZE_TARGET_DIR="//target/symphonize"
"#,
        );
        let sibling = std::path::PathBuf::from(format!("{}-sibling", repo.root.as_str()));
        match symphonize.manifests_dir_for_cwd(&sibling) {
            Err(Error::CurrentDirectoryNotInRepository { cwd, root }) => {
                assert_eq!(sibling.to_string_lossy(), cwd.as_str());
                assert_eq!(repo.root, root);
            }
            Err(err) => panic!("expected CurrentDirectoryNotInRepository; got {err:?}"),
            Ok(path) => panic!("expected CurrentDirectoryNotInRepository; got {path:?}"),
        }
    }

    #[test]
    fn manifests_dir_tracks_repo_relative_cwd() {
        let repo = TempRepo::new();
        let symphonize = symphonize_for(
            &repo,
            r#"
SYMPHONIZE_TARGET_DIR="//target/symphonize"
"#,
        );
        let cwd = std::path::PathBuf::from(repo.root.join("services/api").as_str());
        assert_eq!(
            repo.root.join("target/symphonize/manifests/services/api"),
            symphonize
                .manifests_dir_for_cwd(&cwd)
                .expect("repo subdir should map to manifests subdir")
        );
    }
}
