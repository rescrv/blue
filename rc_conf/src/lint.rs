//! Whole-tree lint of an rc_conf path against an rc.d path.
//!
//! The lint is built on [crate::Why]:  a variable is "read" if any service would consult it while
//! resolving anything its stub, its switch, or the supervisor asks for.  That makes the unread
//! check exactly as precise as the resolver, including alias hops, `_INHERIT` gating, globals, and
//! variables referenced from other values.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::why::{Origin, Provenance};
use crate::{Error, RcConf, SwitchPosition, load_services, var_prefix_from_service};

/// Suffixes that rc_conf itself interprets rather than passing to a stub.
const CONTROL_SUFFIXES: &[&str] = &["ENABLED", "ALIASES", "INHERIT", "AUTOGEN", "SPEC"];

/// Prefixes that rc_conf itself interprets.
const CONTROL_PREFIXES: &[&str] = &["VALUES_", "FILTER_"];

/// Suffixes read by the process supervisor (rcinvoke/rustrc) rather than the stub.
pub const SUPERVISOR_SUFFIXES: &[&str] = &["WRAPPER", "LOG", "STOP_TIMEOUT"];

////////////////////////////////////////////// Severity ////////////////////////////////////////////

/// How bad a finding is.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    /// Something that will not run as written.
    Error,
    /// Almost certainly a mistake, but nothing breaks.
    Warning,
    /// Worth knowing; often intentional.
    Note,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        })
    }
}

////////////////////////////////////////////// Finding /////////////////////////////////////////////

/// One lint finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    /// How bad it is.
    pub severity: Severity,
    /// The service the finding concerns, if any.
    pub service: Option<String>,
    /// Human-readable description.
    pub message: String,
    /// The assignment the finding points at, if any.
    pub origin: Option<Origin>,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(origin) = &self.origin {
            write!(f, "{origin}: ")?;
        }
        write!(f, "{}: ", self.severity)?;
        if let Some(service) = &self.service {
            write!(f, "{service}: ")?;
        }
        f.write_str(&self.message)
    }
}

//////////////////////////////////////////// LintOptions ///////////////////////////////////////////

/// Options for [lint].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LintOptions {
    /// Variable names or suffixes read by tools other than rc.d stubs (e.g. k8src's IMAGE).  A
    /// variable matches if it equals an entry or ends with `_` followed by it.
    pub allow: Vec<String>,
}

impl LintOptions {
    fn allowed(&self, var: &str) -> bool {
        self.allow
            .iter()
            .any(|a| var == a || var.ends_with(&format!("_{a}")))
    }
}

/////////////////////////////////////////////// lint ///////////////////////////////////////////////

/// Lint `rc_conf_path` against `rc_d_path`.  Parse failures are returned as `Err`; everything else
/// is a finding.  Findings are sorted by severity, then file, then line.  This runs every stub's
/// `rcvar` command.
pub fn lint(
    rc_conf_path: &str,
    rc_d_path: &str,
    options: &LintOptions,
) -> Result<Vec<Finding>, Error> {
    let rc_conf = RcConf::parse(rc_conf_path)?;
    let provenance = Provenance::load(rc_conf_path)?;
    let rc_d = load_services(rc_d_path)?;
    let mut findings = vec![];

    // Duplicate assignments within a single file.
    for var in provenance.variables() {
        let assignments = provenance.assignments(var);
        let mut by_file: HashMap<&str, Vec<&Origin>> = HashMap::new();
        for o in assignments {
            by_file.entry(o.path.as_str()).or_default().push(o);
        }
        for (_, dups) in by_file {
            if dups.len() > 1 {
                let last = dups[dups.len() - 1];
                let earlier = dups[..dups.len() - 1]
                    .iter()
                    .map(|o| o.line.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                findings.push(Finding {
                    severity: Severity::Warning,
                    service: None,
                    message: format!("{var} assigned more than once in this file (also line {earlier}); last wins"),
                    origin: Some(last.clone()),
                });
            }
        }
    }

    // Broken stubs.
    let mut stub_names = BTreeSet::new();
    for (name, status) in rc_d.iter() {
        stub_names.insert(name.clone());
        if let Err(err) = status {
            findings.push(Finding {
                severity: Severity::Error,
                service: Some(name.clone()),
                message: format!("rc.d stub unusable: {err}"),
                origin: None,
            });
        }
    }

    let listed = rc_conf.list()?.collect::<BTreeSet<_>>();
    let mut scope = listed.clone();
    scope.extend(stub_names.iter().cloned());

    let mut read: HashSet<String> = HashSet::new();
    let used_stubs = listed
        .iter()
        .map(|s| rc_conf.resolve_alias(s).to_string())
        .collect::<HashSet<_>>();
    for service in scope.iter() {
        let target = rc_conf.resolve_alias(service).to_string();
        let sw = rc_conf.why_switch(&provenance, service);
        for c in sw.candidates.iter() {
            read.insert(c.name.clone());
        }
        if let Some(invalid) = &sw.invalid {
            findings.push(Finding {
                severity: Severity::Error,
                service: Some(service.clone()),
                message: format!(
                    "_ENABLED is {invalid:?}; only YES, NO, and MANUAL are recognized, so it is treated as NO"
                ),
                origin: sw
                    .winner
                    .and_then(|w| sw.candidates[w].origins.last().cloned()),
            });
        }
        let enabled = sw.switch != SwitchPosition::No;
        let stub = match rc_d.get(&target) {
            Some(Ok(path)) => Some(path),
            Some(Err(_)) => None,
            None => {
                if enabled {
                    findings.push(Finding {
                        severity: Severity::Error,
                        service: Some(service.clone()),
                        message: if target == *service {
                            "enabled but there is no executable rc.d stub by this name".to_string()
                        } else {
                            format!(
                                "enabled but its alias target {target} has no executable rc.d stub"
                            )
                        },
                        origin: sw
                            .winner
                            .and_then(|w| sw.candidates[w].origins.last().cloned()),
                    });
                } else if listed.contains(service) && target != *service {
                    findings.push(Finding {
                        severity: Severity::Warning,
                        service: Some(service.clone()),
                        message: format!("aliases {target}, which has no executable rc.d stub"),
                        origin: None,
                    });
                }
                None
            }
        };
        for suffix in SUPERVISOR_SUFFIXES {
            let why = rc_conf.why(&provenance, service, suffix);
            why.names_consulted(&mut read);
            if enabled && let Err(err) = &why.expanded {
                findings.push(Finding {
                    severity: Severity::Error,
                    service: Some(service.clone()),
                    message: format!("{suffix} fails to expand: {err}"),
                    origin: why.winning().and_then(|c| c.origins.last().cloned()),
                });
            }
        }
        let Some(stub) = stub else {
            continue;
        };
        let keys = match rc_conf.stub_rcvars(service, stub) {
            Ok(keys) => keys,
            Err(err) => {
                findings.push(Finding {
                    severity: Severity::Error,
                    service: Some(service.clone()),
                    message: format!("{}: {err}", stub.as_str()),
                    origin: None,
                });
                continue;
            }
        };
        let prefix = var_prefix_from_service(service);
        let mut unset = vec![];
        for key in keys {
            let Some(suffix) = key.strip_prefix(&prefix) else {
                continue;
            };
            let why = rc_conf.why(&provenance, service, suffix);
            why.names_consulted(&mut read);
            if !enabled {
                continue;
            }
            match &why.expanded {
                Ok(None) => unset.push(suffix.to_string()),
                Ok(Some(_)) => {}
                Err(err) => findings.push(Finding {
                    severity: Severity::Error,
                    service: Some(service.clone()),
                    message: format!("{suffix} fails to expand: {err}"),
                    origin: why.winning().and_then(|c| c.origins.last().cloned()),
                }),
            }
        }
        if !unset.is_empty() {
            unset.sort();
            findings.push(Finding {
                severity: Severity::Note,
                service: Some(service.clone()),
                message: format!(
                    "stub reads {} with no value anywhere; each expands empty",
                    unset.join(", ")
                ),
                origin: None,
            });
        }
    }

    // Stubs nothing enables or aliases.
    for stub in stub_names.iter() {
        if !used_stubs.contains(stub) {
            findings.push(Finding {
                severity: Severity::Note,
                service: Some(stub.clone()),
                message: "rc.d stub is never enabled or aliased by rc.conf".to_string(),
                origin: None,
            });
        }
    }

    // Variables nothing reads.
    let mut unread = BTreeMap::new();
    for var in rc_conf.variables() {
        if read.contains(&var)
            || options.allowed(&var)
            || CONTROL_PREFIXES.iter().any(|p| var.starts_with(p))
            || CONTROL_SUFFIXES
                .iter()
                .any(|s| var == *s || var.ends_with(&format!("_{s}")))
        {
            continue;
        }
        unread.insert(var.clone(), provenance.winner(&var).cloned());
    }
    for (var, origin) in unread {
        findings.push(Finding {
            severity: Severity::Warning,
            service: None,
            message: format!(
                "{var} is never read by any service (typo, or read by another tool? see --allow)"
            ),
            origin,
        });
    }

    findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| {
                let pa = a
                    .origin
                    .as_ref()
                    .map(|o| (o.path.as_str().to_string(), o.line));
                let pb = b
                    .origin
                    .as_ref()
                    .map(|o| (o.path.as_str().to_string(), o.line));
                pa.cmp(&pb)
            })
            .then_with(|| a.service.cmp(&b.service))
            .then_with(|| a.message.cmp(&b.message))
    });
    Ok(findings)
}
