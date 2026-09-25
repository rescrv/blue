//! Provenance and resolution explanations for rc_conf variables.
//!
//! [Provenance] records every assignment the parser applied, with file and line.  [Why] explains
//! how a service resolves one variable:  the ordered list of names consulted, which one won, what
//! it expanded to, and (recursively) how each variable referenced by the winning value resolved.
//!
//! The lookup order mirrors [RcConf::variable_provider_for] exactly:
//!
//! 1. Values bound by `_AUTOGEN` expansion (the `VALUES_` variables, e.g. `METRO`), taken from
//!    every hop of the alias chain.
//! 2. `<service>_VAR`, then `<alias>_VAR` for each alias hop, stopping after the first hop that
//!    does not set `_INHERIT="YES"`.
//! 3. The bare `VAR`.
//!
//! `_ENABLED` is resolved differently (every alias hop, no inheritance gate, no global); see
//! [SwitchWhy].

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use utf8path::Path;

use crate::{
    Error, RcConf, SwitchPosition, var_name_from_service, var_prefix_from_service,
    walk_rc_conf_path,
};

/// Recursion limit for explaining variables referenced by a winning value.  Matches the spirit of
/// shvar's own depth limit without depending on its exact value.
const MAX_REFERENCE_DEPTH: usize = 8;

/////////////////////////////////////////////// Origin /////////////////////////////////////////////

/// One assignment as the parser saw it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Origin {
    /// The file containing the assignment.
    pub path: Path<'static>,
    /// The 1-based line on which the (possibly continued) assignment starts.
    pub line: u32,
    /// The value after shell-splitting, before any expansion.
    pub value: String,
}

impl std::fmt::Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.path.as_str(), self.line)
    }
}

///////////////////////////////////////////// Provenance ///////////////////////////////////////////

/// Every assignment reachable from an rc_conf path, in application order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Provenance {
    assignments: HashMap<String, Vec<Origin>>,
}

impl Provenance {
    /// Load provenance for the colon-separated `rc_conf_path`.  This walks the files with the same
    /// traversal [RcConf::parse] uses.
    pub fn load(rc_conf_path: &str) -> Result<Self, Error> {
        let mut assignments: HashMap<String, Vec<Origin>> = HashMap::new();
        walk_rc_conf_path(rc_conf_path, &mut |path, line, var, value| {
            assignments
                .entry(var.to_string())
                .or_default()
                .push(Origin {
                    path: path.clone().into_owned(),
                    line,
                    value,
                });
        })?;
        Ok(Self { assignments })
    }

    /// Every assignment to `var`, earliest first.  The last one is the value in effect.
    pub fn assignments(&self, var: &str) -> &[Origin] {
        self.assignments.get(var).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The assignment to `var` that is in effect, if any.
    pub fn winner(&self, var: &str) -> Option<&Origin> {
        self.assignments(var).last()
    }

    /// All variables with at least one assignment.
    pub fn variables(&self) -> impl Iterator<Item = &str> {
        self.assignments.keys().map(String::as_str)
    }
}

/////////////////////////////////////////////// Layer //////////////////////////////////////////////

/// Where in the lookup order a candidate name sits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Layer {
    /// A value bound by `_AUTOGEN` expansion of the alias `alias` (from a `VALUES_` file).
    Autogen {
        /// The generated alias whose binding supplied the value.
        alias: String,
    },
    /// `<service>_VAR` for the service itself (hop 0) or an alias it inherits through.
    Prefixed {
        /// The service or alias whose prefix was applied.
        service: String,
        /// 0 for the service itself, 1 for the first alias hop, and so on.
        hop: usize,
    },
    /// The bare, un-prefixed variable.
    Global,
}

///////////////////////////////////////////// Candidate ////////////////////////////////////////////

/// One name consulted while resolving a variable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    /// Where this candidate sits in the lookup order.
    pub layer: Layer,
    /// The fully-qualified variable name that was looked up.
    pub name: String,
    /// The raw value, if this name is set.
    pub value: Option<String>,
    /// Every assignment to `name`, earliest first (empty for autogen bindings).
    pub origins: Vec<Origin>,
}

//////////////////////////////////////////////// Why ///////////////////////////////////////////////

/// An explanation of how `service` resolves `var`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Why {
    /// The service the variable was resolved for.
    pub service: String,
    /// The un-prefixed variable name.
    pub var: String,
    /// Every candidate in lookup order.
    pub candidates: Vec<Candidate>,
    /// Index into `candidates` of the one that supplied the value.
    pub winner: Option<usize>,
    /// Why prefixed lookup stopped before reaching the end of the alias chain, if it did.
    pub chain_stop: Option<String>,
    /// The fully expanded value, or the expansion error.
    pub expanded: Result<Option<String>, String>,
    /// Explanations of each variable referenced by the winning value.
    pub references: Vec<Why>,
    /// True if a reference was not explained because it would recurse into a cycle or past the
    /// depth limit.
    pub truncated: bool,
}

impl Why {
    /// The winning candidate, if any.
    pub fn winning(&self) -> Option<&Candidate> {
        self.winner.map(|w| &self.candidates[w])
    }

    /// Every fully-qualified name consulted anywhere in this explanation, references included.
    pub fn names_consulted(&self, into: &mut HashSet<String>) {
        for c in self.candidates.iter() {
            if !matches!(c.layer, Layer::Autogen { .. }) {
                into.insert(c.name.clone());
            }
        }
        for r in self.references.iter() {
            r.names_consulted(into);
        }
    }

    /// Render the explanation as indented text.
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.render_into(&mut out, 0);
        out
    }

    fn render_into(&self, out: &mut String, depth: usize) {
        let pad = "    ".repeat(depth);
        let summary = match &self.expanded {
            Ok(Some(v)) => format!("= {v:?}"),
            Ok(None) => "is unset".to_string(),
            Err(e) => format!("fails to expand: {e}"),
        };
        let _ = writeln!(out, "{pad}{} {} {summary}", self.service, self.var);
        let width = self
            .candidates
            .iter()
            .map(|c| c.name.len())
            .max()
            .unwrap_or(0);
        for (idx, c) in self.candidates.iter().enumerate() {
            let marker = if Some(idx) == self.winner {
                '*'
            } else if c.value.is_some() {
                '~'
            } else {
                ' '
            };
            let layer = match &c.layer {
                Layer::Autogen { alias } => format!("autogen binding of {alias}"),
                Layer::Prefixed { hop: 0, .. } => "service".to_string(),
                Layer::Prefixed { service, hop } => format!("alias hop {hop} ({service})"),
                Layer::Global => "global".to_string(),
            };
            match &c.value {
                Some(value) => {
                    let at = c
                        .origins
                        .last()
                        .map(|o| format!("  {o}"))
                        .unwrap_or_default();
                    let _ = writeln!(
                        out,
                        "{pad}  {marker} {:width$}  {value:?}{at}  [{layer}]",
                        c.name
                    );
                    if c.origins.len() > 1 {
                        for o in c.origins[..c.origins.len() - 1].iter().rev() {
                            let _ = writeln!(
                                out,
                                "{pad}      {:width$}  overrides {:?}  {o}",
                                "", o.value
                            );
                        }
                    }
                }
                None => {
                    let _ = writeln!(out, "{pad}  {marker} {:width$}  unset  [{layer}]", c.name);
                }
            }
        }
        if let Some(stop) = &self.chain_stop {
            let _ = writeln!(out, "{pad}    ({stop})");
        }
        for r in self.references.iter() {
            r.render_into(out, depth + 1);
        }
        if self.truncated {
            let _ = writeln!(
                out,
                "{pad}    (further references elided: cycle or depth limit)"
            );
        }
    }
}

////////////////////////////////////////////// SwitchWhy ///////////////////////////////////////////

/// An explanation of how `service` resolves its `_ENABLED` switch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwitchWhy {
    /// The service.
    pub service: String,
    /// Every `<hop>_ENABLED` consulted, in order.
    pub candidates: Vec<Candidate>,
    /// Index of the candidate that decided the switch.
    pub winner: Option<usize>,
    /// The resulting switch position.
    pub switch: SwitchPosition,
    /// Set when the deciding value is not one of YES, NO, or MANUAL.  The service is treated as
    /// NO.
    pub invalid: Option<String>,
}

impl SwitchWhy {
    /// Render the explanation as indented text.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let switch = match self.switch {
            SwitchPosition::Yes => "YES",
            SwitchPosition::No => "NO",
            SwitchPosition::Manual => "MANUAL",
        };
        let _ = writeln!(out, "{} ENABLED = {switch}", self.service);
        for (idx, c) in self.candidates.iter().enumerate() {
            let marker = if Some(idx) == self.winner { '*' } else { ' ' };
            match &c.value {
                Some(v) => {
                    let at = c
                        .origins
                        .last()
                        .map(|o| format!("  {o}"))
                        .unwrap_or_default();
                    let _ = writeln!(out, "  {marker} {}  {v:?}{at}", c.name);
                }
                None => {
                    let _ = writeln!(out, "  {marker} {}  unset", c.name);
                }
            }
        }
        if let Some(invalid) = &self.invalid {
            let _ = writeln!(
                out,
                "    ({invalid:?} is not YES, NO, or MANUAL; treated as NO)"
            );
        }
        if self.winner.is_none() {
            let _ = writeln!(out, "    (no _ENABLED on any hop; treated as NO)");
        }
        out
    }
}

/////////////////////////////////////////////// RcConf /////////////////////////////////////////////

impl RcConf {
    /// Explain how `service` resolves `var`, attributing each candidate to its assignments in
    /// `provenance` (load it from the same path this RcConf was parsed from).
    pub fn why(&self, provenance: &Provenance, service: &str, var: &str) -> Why {
        let mut stack = HashSet::new();
        self.why_inner(provenance, service, var, 0, &mut stack)
    }

    fn why_inner(
        &self,
        provenance: &Provenance,
        service: &str,
        var: &str,
        depth: usize,
        stack: &mut HashSet<String>,
    ) -> Why {
        let (hops, pre_lookup) = self.alias_hops_with_bindings(service);
        let mut candidates = vec![];

        // 1. autogen bindings
        if let Some((value, alias)) = pre_lookup.get(var) {
            candidates.push(Candidate {
                layer: Layer::Autogen {
                    alias: alias.clone(),
                },
                name: var.to_string(),
                value: Some(value.clone()),
                origins: vec![],
            });
        }

        // 2. prefixed hops, gated on _INHERIT
        let mut chain_stop = None;
        for (hop, name) in hops.iter().enumerate() {
            let full = format!("{}{var}", var_prefix_from_service(name));
            candidates.push(Candidate {
                layer: Layer::Prefixed {
                    service: name.to_string(),
                    hop,
                },
                value: self.items.get(&full).cloned(),
                origins: provenance.assignments(&full).to_vec(),
                name: full,
            });
            let inherit = self
                .aliases
                .get(&var_name_from_service(name))
                .map(|a| a.inherit)
                .unwrap_or(false);
            if !inherit {
                if let Some(next) = hops.get(hop + 1) {
                    chain_stop = Some(format!(
                        "prefixed lookup stops at {name}: {}_INHERIT is not YES, so {next} is not consulted",
                        var_name_from_service(name)
                    ));
                }
                break;
            }
        }

        // 3. global
        candidates.push(Candidate {
            layer: Layer::Global,
            name: var.to_string(),
            value: self.items.get(var).cloned(),
            origins: provenance.assignments(var).to_vec(),
        });

        let winner = candidates.iter().position(|c| c.value.is_some());
        let expanded = match winner.map(|w| candidates[w].value.clone().unwrap_or_default()) {
            Some(raw) => match self.variable_provider_for(service) {
                Ok(vp) => shvar::expand_recursive(&vp, &raw)
                    .map(Some)
                    .map_err(|e| format!("{e:?}")),
                Err(e) => Err(format!("{e}")),
            },
            None => Ok(None),
        };

        let mut references = vec![];
        let mut truncated = false;
        if let Some(raw) = winner.and_then(|w| candidates[w].value.clone())
            && let Ok(mut refs) = shvar::rcvar(&raw)
        {
            refs.sort();
            refs.dedup();
            let key = format!("{service}\0{var}");
            stack.insert(key.clone());
            for r in refs {
                if depth + 1 >= MAX_REFERENCE_DEPTH || stack.contains(&format!("{service}\0{r}")) {
                    truncated = true;
                    continue;
                }
                references.push(self.why_inner(provenance, service, &r, depth + 1, stack));
            }
            stack.remove(&key);
        }

        Why {
            service: service.to_string(),
            var: var.to_string(),
            candidates,
            winner,
            chain_stop,
            expanded,
            references,
            truncated,
        }
    }

    /// Explain how `service` resolves its `_ENABLED` switch.  Mirrors [RcConf::service_switch].
    pub fn why_switch(&self, provenance: &Provenance, service: &str) -> SwitchWhy {
        let (hops, _) = self.alias_hops_with_bindings(service);
        let mut candidates = vec![];
        let mut winner = None;
        let mut switch = SwitchPosition::No;
        let mut invalid = None;
        for name in hops.iter() {
            let full = format!("{}ENABLED", var_prefix_from_service(name));
            let value = self.items.get(&full).cloned();
            let decides = winner.is_none() && value.is_some();
            if decides {
                winner = Some(candidates.len());
                let raw = value.clone().unwrap_or_default();
                let effective = match shvar::split(&raw) {
                    Ok(split) if split.len() == 1 => split[0].clone(),
                    Ok(_) => raw.clone(),
                    Err(_) => {
                        invalid = Some(raw.clone());
                        String::new()
                    }
                };
                if invalid.is_none() {
                    match SwitchPosition::from_enable(&effective) {
                        Some(s) => switch = s,
                        None => invalid = Some(raw.clone()),
                    }
                }
            }
            candidates.push(Candidate {
                layer: Layer::Prefixed {
                    service: name.to_string(),
                    hop: candidates.len(),
                },
                value,
                origins: provenance.assignments(&full).to_vec(),
                name: full,
            });
        }
        SwitchWhy {
            service: service.to_string(),
            candidates,
            winner,
            switch,
            invalid,
        }
    }

    /// The alias chain for `service` (the service first) and the autogen bindings along it, each
    /// attributed to the hop that supplied it.  Mirrors [RcConf::alias_lookup_order].
    fn alias_hops_with_bindings<'a>(
        &'a self,
        service: &'a str,
    ) -> (Vec<&'a str>, HashMap<String, (String, String)>) {
        let mut hops = vec![service];
        let mut direct_alias = service;
        let mut pre_lookup: HashMap<String, (String, String)> = HashMap::new();
        let mut seen = HashSet::from([var_name_from_service(service)]);
        while let Some(alias) = self.aliases.get(&var_name_from_service(direct_alias)) {
            if !seen.insert(var_name_from_service(&alias.aliases)) {
                break;
            }
            for (k, v) in alias.vp.iter() {
                pre_lookup
                    .entry(k.clone())
                    .or_insert_with(|| (v.clone(), direct_alias.to_string()));
            }
            hops.push(&alias.aliases);
            direct_alias = &alias.aliases;
        }
        (hops, pre_lookup)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use shvar::VariableProvider;

    use super::*;

    fn fixture() -> (RcConf, Provenance) {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/rc.conf");
        (
            RcConf::parse(path).unwrap(),
            Provenance::load(path).unwrap(),
        )
    }

    /// The explanation's winner must agree with the resolver it explains, for every service and
    /// every suffix that appears anywhere in the fixture.
    #[test]
    fn why_agrees_with_lookup_suffix() {
        let (rc, prov) = fixture();
        let mut suffixes = HashSet::new();
        for var in rc.variables() {
            let mut pieces = var.split('_').collect::<Vec<_>>();
            while !pieces.is_empty() {
                suffixes.insert(pieces.join("_"));
                pieces.remove(0);
            }
        }
        let mut services = rc.list().unwrap().collect::<Vec<_>>();
        services.push("example1".to_string());
        services.push("nonexistent".to_string());
        for service in services.iter() {
            let vp = rc.variable_provider_for(service).unwrap();
            for suffix in suffixes.iter() {
                let why = rc.why(&prov, service, suffix);
                assert_eq!(
                    why.winning().and_then(|c| c.value.clone()),
                    vp.lookup(suffix),
                    "service={service} suffix={suffix}"
                );
            }
            let sw = rc.why_switch(&prov, service);
            assert_eq!(sw.switch, rc.service_switch(service), "service={service}");
        }
    }

    #[test]
    fn inherited_value_expands_in_the_aliasing_scope() {
        let (rc, prov) = fixture();
        let why = rc.why(&prov, "example3", "FIELD2");
        let winner = why.winning().unwrap();
        assert_eq!(winner.name, "example1_FIELD2");
        assert_eq!(winner.origins.last().unwrap().line, 2);
        // ${TARGET} resolves against example3 first, so it's mars, not world.
        assert_eq!(why.expanded, Ok(Some("mars".to_string())));
        assert_eq!(why.references.len(), 1);
        assert_eq!(why.references[0].winning().unwrap().name, "example3_TARGET");
        assert!(why.render().contains("example3_TARGET"));
    }

    #[test]
    fn autogen_binding_is_attributed() {
        let (rc, prov) = fixture();
        let why = rc.why(&prov, "Jfk_PlanetExpress_example4", "METRO");
        let winner = why.winning().unwrap();
        assert!(matches!(winner.layer, Layer::Autogen { .. }), "{why:?}");
        assert_eq!(winner.value.as_deref(), Some("Jfk"));
    }

    #[test]
    fn provenance_records_overrides_across_files() {
        let dir = std::env::temp_dir().join(format!("rc_conf_why_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.conf");
        let b = dir.join("b.conf");
        std::fs::write(&a, "svc_PORT=\"1\"\nPORT=\"9\"\n").unwrap();
        std::fs::write(&b, "\nsvc_PORT=\"2\"\n").unwrap();
        let path = format!("{}:{}", a.display(), b.display());
        let rc = RcConf::parse(&path).unwrap();
        let prov = Provenance::load(&path).unwrap();
        let why = rc.why(&prov, "svc", "PORT");
        let winner = why.winning().unwrap();
        assert_eq!(winner.value.as_deref(), Some("2"));
        assert_eq!(winner.origins.len(), 2);
        assert_eq!(winner.origins[1].line, 2);
        assert!(winner.origins[1].path.as_str().ends_with("b.conf"));
        assert!(why.render().contains("overrides \"1\""));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn invalid_switch_is_explained() {
        let dir = std::env::temp_dir().join(format!("rc_conf_switch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("rc.conf");
        std::fs::write(&a, "svc_ENABLED=\"yes\"\n").unwrap();
        let path = a.display().to_string();
        let rc = RcConf::parse(&path).unwrap();
        let prov = Provenance::load(&path).unwrap();
        let sw = rc.why_switch(&prov, "svc");
        assert_eq!(sw.switch, SwitchPosition::No);
        assert_eq!(sw.invalid.as_deref(), Some("yes"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn linearize_numbers_logical_lines_from_their_first_physical_line() {
        let lines = crate::linearize(
            &Path::from("x.conf"),
            "A=\"1\"\nB=\"2 \\\n  3\"\n\nC=\"4\"\n",
        )
        .unwrap();
        let numbers = lines
            .iter()
            .map(|(n, l, _)| (*n, l.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            numbers,
            vec![
                (1, "A=\"1\"".to_string()),
                (2, "B=\"2 3\"".to_string()),
                (4, String::new()),
                (5, "C=\"4\"".to_string()),
            ]
        );
    }
}
