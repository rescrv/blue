#![doc = include_str!("../README.md")]

use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;

use handled::SError;
use handled::SExpr;
use serde::Deserialize;
use serde_yaml::{Deserializer, Value, from_str, to_string};
use siphasher::sip128::{Hasher128, SipHasher24};

use rc_conf::RcConf;
use shvar::VariableProvider;
use utf8path::Path;

///////////////////////////////////////////// constants ////////////////////////////////////////////

const K8SIGNORE: &str = ".k8srcignore";

const SERVICE_DEFAULT_YAML: &str = r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${SERVICE:?SERVICE not defined}
  namespace: ${NAMESPACE:?NAMESPACE not defined}
  labels:
    app: ${SERVICE:?SERVICE not defined}
spec:
  replicas: ${REPLICAS:?}
  selector:
    matchLabels:
      app: ${SERVICE:?SERVICE not defined}
  template:
    metadata:
      labels:
        app: ${SERVICE:?SERVICE not defined}
    spec:
      containers:
      - name: ${SERVICE:?SERVICE not defined}
        image: ${IMAGE:?IMAGE not defined}
        ports:
        - containerPort: ${PORT:?PORT not defined}
        envFrom:
        - configMapRef:
            name: ${RCVARS:?RCVARS not defined}
        env:
        - name: RCVAR_ARGV0
          value: ${SERVICE:?SERVICE not defined}
---
apiVersion: v1
kind: Service
metadata:
  name: ${SERVICE:?SERVICE not defined}
  namespace: ${NAMESPACE:?NAMESPACE not defined}
  labels:
    app: ${SERVICE:?SERVICE not defined}
spec:
  type: NodePort
  ports:
  - port: ${PORT:?PORT not defined}
    protocol: TCP
    targetPort: ${PORT:?PORT not defined}
  selector:
    app: ${SERVICE:?SERVICE not defined}
"#;

/// Return the built-in default service template.
pub fn default_service_template() -> &'static str {
    SERVICE_DEFAULT_YAML
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

pub type Error = SError;

const PHASE_REWRITE: &str = "rewrite";
const PHASE_REGENERATE: &str = "regenerate";
const PHASE_TEMPLATE_RESOLUTION: &str = "template-resolution";
const PHASE_WRITE: &str = "write";
const PHASE_RC_CONF_SEARCH: &str = "rc-conf-search";
const PHASE_IMAGE_RCVAR: &str = "image-rcvar";
const PHASE_RC_CONF: &str = "rc-conf";
const PHASE_YAML: &str = "yaml";

const CODE_IO_ERROR: &str = "io-error";
const CODE_INVALID_ROOT: &str = "invalid-root";
const CODE_MISSING_REQUIRED_VARIABLE: &str = "missing-required-variable";
const CODE_COMMAND_FAILED: &str = "command-failed";
const CODE_VERIFICATION_MISMATCH: &str = "verification-mismatch";
const CODE_INVALID_OPTIONS: &str = "invalid-options";
const CODE_NON_UTF8_PATH: &str = "non-utf8-path";
const CODE_PARSE_ERROR: &str = "parse-error";
const CODE_SHVAR_ERROR: &str = "shvar-error";

const FIELD_PATH: &str = "path";
const FIELD_CONTEXT: &str = "context";
const FIELD_OPERATION: &str = "operation";
const FIELD_SERVICE: &str = "service";
const FIELD_TEMPLATE: &str = "template";
const FIELD_RELATIVE: &str = "relative";
const FIELD_OUTPUT: &str = "output";
const FIELD_KEY: &str = "key";
const FIELD_RC_COMMAND: &str = "rc_command";
const FIELD_STATUS: &str = "exit_status";
const FIELD_STDOUT: &str = "stdout";
const FIELD_STDERR: &str = "stderr";
const FIELD_WORKING_DIR: &str = "working_directory";
const FIELD_MESSAGE: &str = "message";
const FIELD_RC_CONF_PATH: &str = "rc_conf_path";

const COMMAND_SAMPLE_LIMIT: usize = 1024;

fn error(phase: &str, code: &str) -> Error {
    SError::new(phase).with_code(code)
}

pub fn error_field<'a>(err: &'a Error, name: &str) -> Option<&'a SExpr> {
    match err.detail() {
        SExpr::List(fields) => fields.iter().find_map(|field| match field {
            SExpr::List(pair) if pair.len() == 2 => match &pair[0] {
                SExpr::Atom(field_name) if field_name == name => Some(&pair[1]),
                _ => None,
            },
            _ => None,
        }),
        _ => None,
    }
}

pub fn error_code(err: &Error) -> Option<&str> {
    match error_field(err, "code") {
        Some(SExpr::Atom(code)) => Some(code.as_str()),
        _ => None,
    }
}

pub fn error_message(err: &Error) -> Option<String> {
    error_string_field(err, FIELD_MESSAGE)
}

pub fn error_string_field(err: &Error, name: &str) -> Option<String> {
    error_field(err, name).map(handled::extract_string)
}

fn error_with_path(err: Error, path: impl AsRef<str>) -> Error {
    err.with_string_field(FIELD_PATH, path.as_ref())
}

fn error_with_context(err: Error, context: impl AsRef<str>) -> Error {
    err.with_string_field(FIELD_CONTEXT, context.as_ref())
}

fn error_with_service(err: Error, service: impl AsRef<str>) -> Error {
    err.with_string_field(FIELD_SERVICE, service.as_ref())
}

fn error_with_template(err: Error, template: impl AsRef<str>) -> Error {
    err.with_string_field(FIELD_TEMPLATE, template.as_ref())
}

fn error_with_rc_conf_path(err: Error, rc_conf_path: impl AsRef<str>) -> Error {
    err.with_string_field(FIELD_RC_CONF_PATH, rc_conf_path.as_ref())
}

fn error_with_relative(err: Error, relative: impl AsRef<str>) -> Error {
    err.with_string_field(FIELD_RELATIVE, relative.as_ref())
}

fn error_with_output(err: Error, output: impl AsRef<str>) -> Error {
    err.with_string_field(FIELD_OUTPUT, output.as_ref())
}

fn wrap_io_error(
    phase: &str,
    err: std::io::Error,
    path: Option<&str>,
    context: &str,
    operation: &str,
) -> Error {
    let mut output = error(phase, CODE_IO_ERROR)
        .with_message(&err.to_string())
        .with_string_field(FIELD_OPERATION, operation);
    output = error_with_context(output, context);
    if let Some(path) = path {
        output = error_with_path(output, path);
    }
    output
}

fn path_exists(phase: &str, path: &Path, context: &str) -> Result<bool, Error> {
    path.exists()
        .map_err(|err| wrap_io_error(phase, err, Some(path.as_str()), context, "exists"))
}

fn wrap_rc_conf_error(err: rc_conf::Error, path: Option<&str>, context: &str) -> Error {
    let mut output = error(PHASE_RC_CONF, CODE_PARSE_ERROR).with_message(&err.to_string());
    output = error_with_context(output, context);
    if let Some(path) = path {
        output = output.with_string_field(FIELD_RC_CONF_PATH, path);
    }
    output
}

fn wrap_shvar_error(err: shvar::Error, context: &str) -> Error {
    match err {
        shvar::Error::Requested(message) => {
            error(PHASE_REWRITE, CODE_PARSE_ERROR).with_message(&message)
        }
        err => error(PHASE_REWRITE, CODE_SHVAR_ERROR)
            .with_message(&err.to_string())
            .with_string_field(FIELD_CONTEXT, context),
    }
}

fn wrap_yaml_error(err: serde_yaml::Error, context: &str) -> Error {
    error(PHASE_YAML, CODE_PARSE_ERROR)
        .with_message(&err.to_string())
        .with_string_field(FIELD_CONTEXT, context)
}

fn sample_snippet(text: &str) -> String {
    text.chars().take(COMMAND_SAMPLE_LIMIT).collect()
}

////////////////////////////////////////////// rewrite /////////////////////////////////////////////

pub fn rewrite(rc_conf: &RcConf, service: &str, yaml: &str) -> Result<String, Error> {
    let vp = rc_conf.variable_provider_for(service).map_err(|err| {
        error_with_context(
            error_with_service(
                wrap_rc_conf_error(err, None, "loading variable provider for requested service"),
                service,
            ),
            "rewrite",
        )
    })?;
    _rewrite(&vp, yaml).map_err(|err| error_with_context(err, "rewriting template"))
}

fn _rewrite(vp: &impl VariableProvider, yaml: &str) -> Result<String, Error> {
    let mut docs = vec![];
    for doc in Deserializer::from_str(yaml) {
        let value = Value::deserialize(doc)
            .map_err(|err| wrap_yaml_error(err, "deserializing template document"))?;
        docs.push(value);
    }
    let docs = docs
        .into_iter()
        .map(|y| transform(vp, y))
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = String::new();
    for doc in docs.into_iter().flatten() {
        if !out.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out += "---\n";
        }
        out += &to_string(&doc)
            .map_err(|err| wrap_yaml_error(err, "serializing rewritten template"))?;
    }
    Ok(out)
}

fn transform(vp: &dyn VariableProvider, yaml: Value) -> Result<Option<Value>, Error> {
    fn transform_vec(vp: &dyn VariableProvider, yaml: Vec<Value>) -> Result<Option<Value>, Error> {
        let yaml = yaml
            .into_iter()
            .map(|y| transform(vp, y))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Value::Sequence(yaml.into_iter().flatten().collect())))
    }

    fn transform_kv(
        vp: &dyn VariableProvider,
        k: Value,
        v: Value,
    ) -> Result<Option<(Value, Value)>, Error> {
        let k = transform(vp, k)?;
        let v = transform(vp, v)?;
        if let (Some(k), Some(v)) = (k, v) {
            Ok(Some((k, v)))
        } else {
            Ok(None)
        }
    }

    fn transform_hash(
        vp: &dyn VariableProvider,
        yaml: impl Iterator<Item = (Value, Value)>,
    ) -> Result<Option<Value>, Error> {
        let yaml = yaml
            .into_iter()
            .map(|(k, v)| transform_kv(vp, k, v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Value::Mapping(yaml.into_iter().flatten().collect())))
    }

    match yaml {
        Value::String(s) => match shvar::expand_recursive(vp, &s) {
            Ok(expanded) => {
                let pieces = shvar::split(&expanded)
                    .map_err(|err| wrap_shvar_error(err, "splitting expanded template"))?;
                let quoted = shvar::quote(pieces);
                let value: Value = from_str(&quoted)
                    .map_err(|err| wrap_yaml_error(err, "parsing rewritten template fragment"))?;
                Ok(Some(value))
            }
            Err(shvar::Error::Requested(msg)) => {
                if msg.is_empty() {
                    Ok(None)
                } else {
                    Err(error_with_context(
                        wrap_shvar_error(
                            shvar::Error::Requested(msg),
                            "required variable expansion",
                        ),
                        "template expansion",
                    ))
                }
            }
            Err(err) => Err(wrap_shvar_error(err, "template expansion")),
        },
        Value::Sequence(a) => transform_vec(vp, a),
        Value::Mapping(h) => transform_hash(vp, h.into_iter()),
        _ => Ok(Some(yaml)),
    }
}

///////////////////////////////////////// RegenerateOptions ////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct RegenerateOptions {
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Root of the k8src repository.")
    )]
    pub root: Option<String>,
    #[cfg_attr(feature = "command_line", arrrg(optional, "Root of the k8src output."))]
    pub output: Option<String>,
    #[cfg_attr(feature = "command_line", arrrg(flag, "Overwrite the existing files."))]
    pub overwrite: bool,
    #[cfg_attr(
        feature = "command_line",
        arrrg(flag, "Verify the existing files rather than generating.")
    )]
    pub verify: bool,
    #[cfg_attr(
        feature = "command_line",
        arrrg(flag, "Print generated manifest paths without writing files.")
    )]
    pub dry_run: bool,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            flag,
            "Print unified diffs for changed manifests without writing files."
        )
    )]
    pub diff: bool,
}

//////////////////////////////////////////// regenerate ////////////////////////////////////////////

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}

fn unified_diff(path: &str, old: Option<&str>, new: &str) -> String {
    let old_path = if old.is_some() { path } else { "/dev/null" };
    let old = old.unwrap_or("");
    if old == new {
        return String::new();
    }
    let mut out = String::new();
    out += &format!("--- {old_path}\n");
    out += &format!("+++ {path}\n");
    out += &format!("@@ -1,{} +1,{} @@\n", line_count(old), line_count(new));
    for line in old.lines() {
        out += &format!("-{line}\n");
    }
    for line in new.lines() {
        out += &format!("+{line}\n");
    }
    out
}

fn write_yaml(
    options: &RegenerateOptions,
    output: Path,
    yaml: &str,
    tracking: &mut Vec<Path>,
) -> Result<(), Error> {
    if options.verify && options.overwrite {
        Err(error(PHASE_WRITE, CODE_INVALID_OPTIONS)
            .with_message("--verify and --overwrite are mutually exclusive")
            .with_string_field(FIELD_OUTPUT, output.as_str()))
    } else if options.verify {
        if !path_exists(PHASE_WRITE, &output, "checking manifest for verification")? {
            return Err(error(PHASE_WRITE, CODE_VERIFICATION_MISMATCH)
                .with_message("manifest is missing for verification")
                .with_string_field(FIELD_OUTPUT, output.as_str()));
        }
        let returned = yaml;
        let expected = std::fs::read_to_string(&output).map_err(|err| {
            wrap_io_error(
                PHASE_WRITE,
                err,
                Some(output.as_str()),
                "reading manifest for verification",
                "read_to_string",
            )
        })?;
        if expected != returned {
            Err(error(PHASE_WRITE, CODE_VERIFICATION_MISMATCH)
                .with_message("manifest contents differ during verification")
                .with_string_field(FIELD_OUTPUT, output.as_str())
                .with_string_field("expected_length", &expected.len().to_string())
                .with_string_field("returned_length", &returned.len().to_string()))
        } else {
            tracking.push(output.into_owned());
            Ok(())
        }
    } else if options.diff {
        let expected = if path_exists(PHASE_WRITE, &output, "checking manifest for diff")? {
            Some(std::fs::read_to_string(&output).map_err(|err| {
                wrap_io_error(
                    PHASE_WRITE,
                    err,
                    Some(output.as_str()),
                    "reading manifest for diff",
                    "read_to_string",
                )
            })?)
        } else {
            None
        };
        let diff = unified_diff(output.as_str(), expected.as_deref(), yaml);
        if !diff.is_empty() {
            print!("{diff}");
        }
        tracking.push(output.into_owned());
        Ok(())
    } else if options.dry_run {
        println!("would generate {}", output.as_str());
        tracking.push(output.into_owned());
        Ok(())
    } else {
        if path_exists(PHASE_WRITE, &output, "checking generated manifest")? && !options.overwrite {
            return Err(error(PHASE_WRITE, CODE_INVALID_OPTIONS)
                .with_message("manifest already exists and overwrite is disabled")
                .with_string_field(FIELD_OUTPUT, output.as_str()));
        }
        std::fs::create_dir_all(output.dirname()).map_err(|err| {
            wrap_io_error(
                PHASE_WRITE,
                err,
                Some(output.dirname().as_str()),
                "creating output directory",
                "create_dir_all",
            )
        })?;
        std::fs::write(&output, yaml).map_err(|err| {
            wrap_io_error(
                PHASE_WRITE,
                err,
                Some(output.as_str()),
                "writing generated manifest",
                "write",
            )
        })?;
        tracking.push(output.into_owned());
        Ok(())
    }
}

pub fn regenerate(options: RegenerateOptions) -> Result<(), Error> {
    let root = if let Some(root) = options.root.as_ref() {
        Path::from(root)
    } else {
        Path::cwd().ok_or_else(|| {
            error(PHASE_REGENERATE, CODE_INVALID_ROOT).with_message("current directory unavailable")
        })?
    };
    let output = if let Some(output) = options.output.as_ref() {
        Path::from(output)
    } else {
        root.join("manifests")
    };
    if !options.verify
        && !options.overwrite
        && !options.dry_run
        && !options.diff
        && path_exists(
            PHASE_REGENERATE,
            &output,
            "checking manifests output directory",
        )?
    {
        return Err(error(PHASE_REGENERATE, CODE_INVALID_OPTIONS)
            .with_message("manifests output directory exists and overwrite is disabled")
            .with_string_field(FIELD_OUTPUT, output.as_str()));
    }
    if options.overwrite
        && !options.dry_run
        && !options.diff
        && path_exists(
            PHASE_REGENERATE,
            &output,
            "checking manifests output directory for overwrite",
        )?
    {
        std::fs::remove_dir_all(&output).map_err(|err| {
            wrap_io_error(
                PHASE_WRITE,
                err,
                Some(output.as_str()),
                "removing existing manifests directory",
                "remove_dir_all",
            )
        })?;
    }
    let rc_confs = restrict_to_terminals(find_rc_confs(&root)?);
    for rc_conf in rc_confs.into_iter() {
        let candidates = candidates(&root, &rc_conf)?;
        let Some(candidate_root) = candidates
            .last()
            .and_then(|candidate| candidate.strip_prefix(root.as_str()))
        else {
            return Err(error(PHASE_REGENERATE, CODE_INVALID_ROOT)
                .with_message("rc_conf is not under regeneration root")
                .with_string_field(FIELD_OUTPUT, output.as_str())
                .with_string_field(FIELD_CONTEXT, "candidate path derivation")
                .with_string_field(FIELD_RC_CONF_PATH, rc_conf_path(&candidates).as_str()));
        };
        let relative = Path::from(candidate_root.as_str().trim_start_matches('/'));
        let rc_conf_chain = rc_conf_path(&candidates);
        let rc_conf = RcConf::parse(&rc_conf_chain).map_err(|err| {
            error_with_rc_conf_path(
                wrap_rc_conf_error(err, Some(&rc_conf_chain), "parsing rc.conf chain"),
                rc_conf_chain.as_str(),
            )
        })?;
        let mut root_yaml = r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
"#
        .to_string();
        let mut extended = false;
        let mut tracking = vec![];
        for service in rc_conf.list_services().map_err(|err| {
            error_with_rc_conf_path(
                wrap_rc_conf_error(err, Some(&rc_conf_chain), "listing services"),
                rc_conf_chain.as_str(),
            )
        })? {
            let Some(image) = rc_conf.lookup_suffix(&service, "IMAGE") else {
                return Err(error(PHASE_REGENERATE, CODE_MISSING_REQUIRED_VARIABLE)
                    .with_message("IMAGE is required")
                    .with_string_field(FIELD_SERVICE, &service)
                    .with_string_field(FIELD_KEY, "IMAGE")
                    .with_string_field(FIELD_OUTPUT, output.as_str())
                    .with_string_field(FIELD_RC_CONF_PATH, rc_conf_chain.as_str())
                    .with_string_field(FIELD_RELATIVE, relative.as_str()));
            };
            let extra =
                HashMap::from_iter([("IMAGE", image.clone()), ("RCVAR_ARGV0", service.clone())]);
            let rcvar = rc_conf
                .argv(&service, "IMAGE_RCVAR", &extra)
                .map_err(|err| {
                    error_with_service(
                        wrap_rc_conf_error(
                            err,
                            Some(&rc_conf_chain),
                            "expanding IMAGE_RCVAR for service",
                        ),
                        &service,
                    )
                })?;
            let rcvars = if !rcvar.is_empty() {
                let working_directory = std::env::current_dir().map_err(|err| {
                    wrap_io_error(
                        PHASE_IMAGE_RCVAR,
                        err,
                        None,
                        "looking up working directory for IMAGE_RCVAR command",
                        "current_dir",
                    )
                })?;
                let working_directory = working_directory.to_string_lossy().into_owned();
                let mut command = std::process::Command::new(&rcvar[0]);
                command.args(&rcvar[1..]);
                command.current_dir(&working_directory);
                let command_line = rcvar.join(" ");
                let rcvar = command.output().map_err(|err| {
                    wrap_io_error(
                        PHASE_IMAGE_RCVAR,
                        err,
                        Some(&rc_conf_chain),
                        "running IMAGE_RCVAR command",
                        "output",
                    )
                    .with_string_field(FIELD_SERVICE, &service)
                    .with_string_field(FIELD_RC_CONF_PATH, rc_conf_chain.as_str())
                    .with_string_field(FIELD_OUTPUT, output.as_str())
                    .with_string_field(FIELD_RC_COMMAND, &command_line)
                    .with_string_field(FIELD_WORKING_DIR, &working_directory)
                    .with_string_field(FIELD_CONTEXT, "running IMAGE_RCVAR command")
                })?;
                if !rcvar.status.success() {
                    let stdout = sample_snippet(&String::from_utf8_lossy(&rcvar.stdout));
                    let stderr = sample_snippet(&String::from_utf8_lossy(&rcvar.stderr));
                    let status = rcvar.status.to_string();
                    return Err(error(PHASE_IMAGE_RCVAR, CODE_COMMAND_FAILED)
                        .with_message("IMAGE_RCVAR command failed")
                        .with_string_field(FIELD_SERVICE, &service)
                        .with_string_field(FIELD_RC_CONF_PATH, rc_conf_chain.as_str())
                        .with_string_field(FIELD_RC_COMMAND, &command_line)
                        .with_string_field(FIELD_WORKING_DIR, &working_directory)
                        .with_string_field(FIELD_STATUS, &status)
                        .with_string_field(FIELD_STDOUT, &stdout)
                        .with_string_field(FIELD_STDERR, &stderr)
                        .with_string_field(FIELD_CONTEXT, "running IMAGE_RCVAR command"));
                }
                let rckeys = String::from_utf8_lossy(&rcvar.stdout);
                let mut rckeys = rckeys
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                rckeys.sort();
                let mut rcvars = rc_conf
                    .generate_rcvars(&service, &rckeys)
                    .map_err(|err| {
                        error_with_service(
                            wrap_rc_conf_error(
                                err,
                                Some(&rc_conf_chain),
                                "collecting IMAGE_RCVAR expansion variables",
                            ),
                            &service,
                        )
                        .with_string_field(FIELD_OUTPUT, output.as_str())
                        .with_string_field(FIELD_RC_CONF_PATH, rc_conf_chain.as_str())
                    })?
                    .into_iter()
                    .collect::<Vec<_>>();
                rcvars.sort();
                rcvars
            } else {
                vec![]
            };
            let (template, mut yaml) = match template_for_service(&candidates, &rc_conf, &service) {
                Some(template) => (
                    template.as_str().to_string(),
                    std::fs::read_to_string(&template).map_err(|err| {
                        wrap_io_error(
                            PHASE_TEMPLATE_RESOLUTION,
                            err,
                            Some(template.as_str()),
                            "reading template file",
                            "read_to_string",
                        )
                        .with_string_field(FIELD_SERVICE, &service)
                        .with_string_field(FIELD_OUTPUT, output.as_str())
                        .with_string_field(FIELD_RC_CONF_PATH, rc_conf_chain.as_str())
                        .with_string_field(FIELD_CONTEXT, "template_for_service")
                    })?,
                ),
                None => (
                    "default-template".to_string(),
                    SERVICE_DEFAULT_YAML.to_string(),
                ),
            };
            const MAGIC_KEY: &[u8; 16] = &[
                173, 14, 53, 145, 150, 207, 208, 116, 119, 25, 149, 255, 4, 53, 29, 50,
            ];
            let mut hasher = SipHasher24::new_with_key(MAGIC_KEY);
            service.hash(&mut hasher);
            rcvars.hash(&mut hasher);
            let sig = hasher.finish128().as_u128();
            let mut config_map = format!(
                r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: config-map-{sig}
  namespace: ${{NAMESPACE:?NAMESPACE not defined}}
data:
"#
            );
            for (key, value) in rcvars {
                config_map += &format!("  {key}: {value:?}");
            }
            if !yaml.is_empty() && !yaml.ends_with('\n') {
                yaml.push('\n');
            }
            yaml += "---\n";
            yaml += &config_map;
            let rcvp = rc_conf.variable_provider_for(&service).map_err(|err| {
                error_with_service(
                    wrap_rc_conf_error(
                        err,
                        Some(&rc_conf_chain),
                        "creating service variable provider",
                    ),
                    &service,
                )
            })?;
            let locals = HashMap::from_iter([
                ("SERVICE", service.clone()),
                ("RCVARS", format!("config-map-{sig}")),
            ]);
            let vp = (&locals, &rcvp);
            let yaml = _rewrite(&vp, &yaml)
                .map_err(|err| error_with_template(error_with_service(err, &service), &template))?;
            let service_output = output.join(Path::from(format!("{relative}/herd/{service}.yaml")));
            write_yaml(&options, service_output, &yaml, &mut tracking).map_err(|err| {
                error_with_template(
                    error_with_service(
                        error_with_rc_conf_path(err, rc_conf_chain.as_str()),
                        &service,
                    ),
                    template.as_str(),
                )
            })?;
            root_yaml += &format!("- {service}.yaml\n");
            extended = true;
        }
        if extended {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/herd/kustomization.yaml"))),
                &root_yaml,
                &mut tracking,
            )
            .map_err(|err| {
                error_with_rc_conf_path(
                    error_with_relative(error_with_output(err, output.as_str()), relative.as_str()),
                    rc_conf_chain.as_str(),
                )
            })?;
        }
        let mut have_pets = false;
        for candidate in candidates.iter().rev() {
            let pets = candidate.join("pets");
            if !path_exists(PHASE_REGENERATE, &pets, "checking pets directory")? {
                continue;
            }
            fn copy_pets_from_dir(
                options: &RegenerateOptions,
                root: &Path,
                output: &Path,
                pets: &Path,
                rc_conf_chain: &str,
                tracking: &mut Vec<Path>,
            ) -> Result<bool, Error> {
                let mut copied = false;
                for pet in std::fs::read_dir(pets).map_err(|err| {
                    wrap_io_error(
                        PHASE_RC_CONF_SEARCH,
                        err,
                        Some(pets.as_str()),
                        "reading pets directory",
                        "read_dir",
                    )
                })? {
                    let pet = pet.map_err(|err| {
                        wrap_io_error(
                            PHASE_RC_CONF_SEARCH,
                            err,
                            Some(pets.as_str()),
                            "reading pet directory entry",
                            "next",
                        )
                    })?;
                    let pet = Path::try_from(pet.path()).map_err(|_| {
                        error(PHASE_RC_CONF_SEARCH, CODE_NON_UTF8_PATH)
                            .with_message("non-UTF8 pet path encountered during copy")
                            .with_string_field(FIELD_RC_CONF_PATH, rc_conf_chain)
                    })?;
                    if path_exists(
                        PHASE_RC_CONF_SEARCH,
                        &pet.join(K8SIGNORE),
                        "checking pet .k8srcignore",
                    )? {
                        continue;
                    }
                    if pet.is_dir().map_err(|err| {
                        wrap_io_error(
                            PHASE_RC_CONF_SEARCH,
                            err,
                            Some(pet.as_str()),
                            "checking pet directory",
                            "is_dir",
                        )
                    })? {
                        copied |= copy_pets_from_dir(
                            options,
                            root,
                            output,
                            &pet,
                            rc_conf_chain,
                            tracking,
                        )?;
                        continue;
                    }
                    if !pet.as_str().ends_with(".yaml") {
                        eprintln!("skipping pet {pet:?}");
                        continue;
                    }
                    let Some(relative) = pet.strip_prefix(root.as_str()) else {
                        return Err(error(PHASE_REGENERATE, CODE_INVALID_ROOT)
                            .with_message("pet is not inside repository root")
                            .with_string_field(FIELD_RC_CONF_PATH, rc_conf_chain)
                            .with_string_field(FIELD_CONTEXT, "copying pets")
                            .with_string_field(FIELD_OUTPUT, root.as_str()));
                    };
                    let relative = Path::from(relative.as_str().trim_start_matches('/'));
                    let source = pet.clone();
                    let output = output.join(Path::from(format!("{relative}")));
                    let contents = std::fs::read_to_string(source.as_str()).map_err(|err| {
                        wrap_io_error(
                            PHASE_WRITE,
                            err,
                            Some(source.as_str()),
                            "reading pet file",
                            "read_to_string",
                        )
                    })?;
                    write_yaml(options, output, &contents, tracking)?;
                    copied = true;
                }
                Ok(copied)
            }
            have_pets |= copy_pets_from_dir(
                &options,
                &root,
                &output,
                &pets,
                rc_conf_chain.as_str(),
                &mut tracking,
            )?;
        }
        if extended && have_pets {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/kustomization.yaml"))),
                r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - herd
  - pets
"#,
                &mut tracking,
            )
            .map_err(|err| {
                error_with_rc_conf_path(
                    error_with_relative(error_with_output(err, output.as_str()), relative.as_str()),
                    rc_conf_chain.as_str(),
                )
            })?;
        } else if extended {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/kustomization.yaml"))),
                r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - herd
"#,
                &mut tracking,
            )
            .map_err(|err| {
                error_with_rc_conf_path(
                    error_with_relative(error_with_output(err, output.as_str()), relative.as_str()),
                    rc_conf_chain.as_str(),
                )
            })?;
        } else if have_pets {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/kustomization.yaml"))),
                r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - pets
"#,
                &mut tracking,
            )
            .map_err(|err| {
                error_with_rc_conf_path(
                    error_with_relative(error_with_output(err, output.as_str()), relative.as_str()),
                    rc_conf_chain.as_str(),
                )
            })?;
        }
    }
    Ok(())
}

fn find_rc_confs(root: &Path) -> Result<Vec<Path<'static>>, Error> {
    find_rc_confs_inner(root, true)
}

fn find_rc_confs_inner(root: &Path, is_root: bool) -> Result<Vec<Path<'static>>, Error> {
    let mut paths = vec![];
    if !is_root
        && path_exists(
            PHASE_RC_CONF_SEARCH,
            &root.join(K8SIGNORE),
            "checking .k8srcignore",
        )?
    {
        return Ok(paths);
    }
    for dirent in std::fs::read_dir(root).map_err(|err| {
        wrap_io_error(
            PHASE_RC_CONF_SEARCH,
            err,
            Some(root.as_str()),
            "scanning filesystem for rc_conf files",
            "read_dir",
        )
    })? {
        let dirent = dirent.map_err(|err| {
            wrap_io_error(
                PHASE_RC_CONF_SEARCH,
                err,
                Some(root.as_str()),
                "reading directory entry",
                "next",
            )
        })?;
        let path = Path::try_from(dirent.path()).map_err(|_| {
            error(PHASE_RC_CONF_SEARCH, CODE_NON_UTF8_PATH)
                .with_message("non-UTF8 directory entry path encountered")
                .with_string_field(FIELD_PATH, &dirent.path().to_string_lossy())
        })?;
        if dirent.file_name() == "rc.conf" {
            paths.push(path.clone());
        }
        if dirent
            .file_type()
            .map_err(|err| {
                wrap_io_error(
                    PHASE_RC_CONF_SEARCH,
                    err,
                    Some(path.as_str()),
                    "stat'ing directory entry",
                    "file_type",
                )
            })?
            .is_dir()
        {
            let children = find_rc_confs_inner(&path, false)?;
            paths.extend(children);
        }
    }
    Ok(paths)
}

fn restrict_to_terminals(mut rc_confs: Vec<Path<'static>>) -> Vec<Path<'static>> {
    rc_confs.sort_by_key(|rc_conf| rc_conf.as_str().len());
    let mut restricted: Vec<Path> = vec![];
    for rc_conf in rc_confs.into_iter().rev() {
        fn is_parent_rc_conf(parent: &Path, child: &Path) -> bool {
            let parent = parent.dirname();
            let mut child = child.clone();
            while child.components().count() > parent.components().count() {
                child = child.dirname().into_owned();
            }
            child == parent
        }
        if !restricted.iter().any(|r| is_parent_rc_conf(&rc_conf, r)) {
            restricted.push(rc_conf);
        }
    }
    restricted
}

pub fn candidates(root: &Path, rc_conf: &Path) -> Result<Vec<Path<'static>>, Error> {
    if !rc_conf.as_str().starts_with(root.as_str()) {
        return Err(error(PHASE_REGENERATE, CODE_INVALID_ROOT)
            .with_message("rc_conf is not under root")
            .with_string_field(FIELD_PATH, root.as_str())
            .with_string_field(FIELD_RC_CONF_PATH, rc_conf.as_str()));
    }
    let mut rc_conf = rc_conf.dirname();
    let mut candidates = vec![];
    while rc_conf != *root {
        candidates.push(rc_conf.clone().into_owned());
        rc_conf = rc_conf.dirname().into_owned();
    }
    candidates.push(root.clone().into_owned());
    candidates.reverse();
    Ok(candidates)
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateCandidate {
    pub path: String,
    pub exists: bool,
    pub service: Option<String>,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateResolution {
    pub selected: Option<String>,
    pub fallback_chain: Vec<TemplateCandidate>,
    pub uses_builtin_default: bool,
}

pub fn template_resolution(
    candidates: &[Path],
    rc_conf: &RcConf,
    service: &str,
) -> TemplateResolution {
    let mut service = service.to_string();
    let mut fallback_chain = vec![];
    loop {
        for candidate in candidates.iter().rev() {
            let candidate = candidate
                .join("rc.d")
                .join(format!("{service}.yaml.template"));
            let path = candidate.as_str().to_string();
            let exists = candidate.exists().unwrap_or(false);
            fallback_chain.push(TemplateCandidate {
                path: path.clone(),
                exists,
                service: Some(service.clone()),
                kind: "service-specific".to_string(),
            });
            if exists {
                return TemplateResolution {
                    selected: Some(path),
                    fallback_chain,
                    uses_builtin_default: false,
                };
            }
        }
        let direct_alias = rc_conf.direct_alias(&service);
        if direct_alias == service {
            break;
        } else {
            service = direct_alias.to_string();
        }
    }
    for candidate in candidates.iter().rev() {
        let candidate = candidate.join("service.yaml.template");
        let path = candidate.as_str().to_string();
        let exists = candidate.exists().unwrap_or(false);
        fallback_chain.push(TemplateCandidate {
            path: path.clone(),
            exists,
            service: None,
            kind: "default".to_string(),
        });
        if exists {
            return TemplateResolution {
                selected: Some(path),
                fallback_chain,
                uses_builtin_default: false,
            };
        }
    }
    TemplateResolution {
        selected: None,
        fallback_chain,
        uses_builtin_default: true,
    }
}

fn template_for_service(
    candidates: &[Path],
    rc_conf: &RcConf,
    service: &str,
) -> Option<Path<'static>> {
    template_resolution(candidates, rc_conf, service)
        .selected
        .map(Path::from)
}

fn explain_context(root: &Path) -> Result<(Vec<Path<'static>>, String, RcConf), Error> {
    let rc_confs = restrict_to_terminals(find_rc_confs(root)?);
    if rc_confs.is_empty() {
        return Err(error(PHASE_RC_CONF_SEARCH, CODE_INVALID_ROOT)
            .with_message("no rc.conf found under root")
            .with_string_field(FIELD_PATH, root.as_str()));
    }
    if rc_confs.len() != 1 {
        let rc_conf_path = rc_confs
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(":");
        return Err(error(PHASE_RC_CONF_SEARCH, CODE_INVALID_OPTIONS)
            .with_message("multiple terminal rc.conf files found; run from one overlay root")
            .with_string_field(FIELD_RC_CONF_PATH, &rc_conf_path));
    }
    let candidates = candidates(root, &rc_confs[0])?;
    let rc_conf_chain = rc_conf_path(&candidates);
    let rc_conf = RcConf::parse(&rc_conf_chain)
        .map_err(|err| wrap_rc_conf_error(err, Some(&rc_conf_chain), "parsing rc.conf chain"))?;
    Ok((candidates, rc_conf_chain, rc_conf))
}

pub fn explain_template(root: Option<&str>, service: &str) -> Result<String, Error> {
    let root = if let Some(root) = root {
        Path::from(root)
    } else {
        Path::cwd().ok_or_else(|| {
            error(PHASE_TEMPLATE_RESOLUTION, CODE_INVALID_ROOT)
                .with_message("current directory unavailable")
        })?
    };
    let (candidates, rc_conf_chain, rc_conf) = explain_context(&root)?;
    let resolution = template_resolution(&candidates, &rc_conf, service);
    let selected = resolution
        .selected
        .as_deref()
        .unwrap_or("built-in default template");
    let mut out = String::new();
    out += &format!("service: {service}\n");
    out += &format!("rc_conf_path: {rc_conf_chain}\n");
    out += &format!("selected: {selected}\n");
    out += "fallback_chain:\n";
    for candidate in resolution.fallback_chain {
        let marker = if candidate.exists { "found" } else { "missing" };
        out += &format!("  - {marker} {} ({})\n", candidate.path, candidate.kind);
    }
    if resolution.uses_builtin_default {
        out += "  - found built-in default template\n";
    }
    Ok(out)
}

pub fn explain_vars(root: Option<&str>, service: &str) -> Result<String, Error> {
    let root = if let Some(root) = root {
        Path::from(root)
    } else {
        Path::cwd().ok_or_else(|| {
            error(PHASE_RC_CONF, CODE_INVALID_ROOT).with_message("current directory unavailable")
        })?
    };
    let (_candidates, rc_conf_chain, rc_conf) = explain_context(&root)?;
    let (alias_order, pre_lookup) = rc_conf.alias_lookup_order(service);
    let vp = rc_conf.variable_provider_for(service).map_err(|err| {
        error_with_service(
            wrap_rc_conf_error(err, Some(&rc_conf_chain), "loading variable provider"),
            service,
        )
    })?;
    let known_prefixes = rc_conf
        .list()
        .map_err(|err| wrap_rc_conf_error(err, Some(&rc_conf_chain), "listing services"))?
        .map(|service| rc_conf::var_prefix_from_service(&service))
        .collect::<Vec<_>>();
    let alias_prefixes = alias_order
        .iter()
        .map(|service| rc_conf::var_prefix_from_service(service))
        .collect::<Vec<_>>();
    let mut suffixes = BTreeSet::new();
    for raw in rc_conf.variables() {
        let mut matched = false;
        for prefix in &alias_prefixes {
            if let Some(suffix) = raw.strip_prefix(prefix) {
                suffixes.insert(suffix.to_string());
                matched = true;
            }
        }
        if matched {
            continue;
        }
        if known_prefixes.iter().any(|prefix| raw.starts_with(prefix)) {
            continue;
        }
        suffixes.insert(raw);
    }
    suffixes.extend(pre_lookup.keys().cloned());
    let mut out = String::new();
    out += &format!("service: {service}\n");
    out += &format!("rc_conf_path: {rc_conf_chain}\n");
    out += &format!("alias_order: {}\n", alias_order.join(" -> "));
    out += "variables:\n";
    for suffix in suffixes {
        if let Some(value) = vp.lookup(&suffix) {
            let value = shvar::expand_recursive(&vp, &value).unwrap_or(value);
            out += &format!("  {suffix}={}\n", shvar::quote_string(&value));
        }
    }
    Ok(out)
}
