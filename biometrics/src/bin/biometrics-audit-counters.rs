use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use serde::Deserialize;
use syn::meta::ParseNestedMeta;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprMethodCall, File, Item, ItemStatic, Lit, Path as SynPath};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Package {
    name: String,
    path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CounterDefinition {
    ident: String,
    label: String,
    package_path: PathBuf,
    file: PathBuf,
    line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MissingCounter {
    label: String,
    package_path: PathBuf,
    file: PathBuf,
    line: usize,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct FileAudit {
    counters: Vec<CounterDefinition>,
    registered: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: Option<CargoPackage>,
    workspace: Option<CargoWorkspace>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CargoWorkspace {
    #[serde(default)]
    members: Vec<String>,
}

fn main() {
    let root = match env::current_dir() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("error: failed to read current directory: {err}");
            process::exit(2);
        }
    };
    let args = env::args().skip(1);
    match run(&root, args, &mut io::stdout()) {
        Ok(status) => process::exit(status),
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(2);
        }
    }
}

fn run<I, W>(root: &Path, args: I, output: &mut W) -> Result<i32, String>
where
    I: IntoIterator<Item = String>,
    W: Write,
{
    let package = parse_args(args)?;
    let missing = audit_workspace(root, package.as_deref())?;
    for counter in &missing {
        writeln!(
            output,
            "{}\t{}\t{}",
            counter.label,
            path_to_string(&counter.file),
            counter.line
        )
        .map_err(|err| format!("failed to write audit output: {err}"))?;
    }
    Ok(if missing.is_empty() { 0 } else { 1 })
}

fn parse_args<I>(args: I) -> Result<Option<String>, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let mut package = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-p" | "--package" => {
                if package.is_some() {
                    return Err(format!("package specified more than once\n{}", usage()));
                }
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing package name after {arg}\n{}", usage()))?;
                if value.is_empty() {
                    return Err(format!("empty package name after {arg}\n{}", usage()));
                }
                package = Some(value);
            }
            "-h" | "--help" => {
                return Err(usage().to_string());
            }
            _ => {
                return Err(format!("unexpected argument: {arg}\n{}", usage()));
            }
        }
    }
    Ok(package)
}

fn usage() -> &'static str {
    "usage: biometrics-audit-counters [-p <package>]"
}

fn audit_workspace(root: &Path, package: Option<&str>) -> Result<Vec<MissingCounter>, String> {
    let packages = select_packages(load_workspace_packages(root)?, package)?;
    let mut missing = Vec::new();
    for package in packages {
        missing.extend(audit_package(root, &package)?);
    }
    missing.sort_by(|lhs, rhs| {
        lhs.package_path
            .cmp(&rhs.package_path)
            .then_with(|| lhs.file.cmp(&rhs.file))
            .then_with(|| lhs.line.cmp(&rhs.line))
            .then_with(|| lhs.label.cmp(&rhs.label))
    });
    Ok(missing)
}

fn select_packages(packages: Vec<Package>, package: Option<&str>) -> Result<Vec<Package>, String> {
    if let Some(package) = package {
        let matches: Vec<Package> = packages
            .into_iter()
            .filter(|candidate| candidate.name == package)
            .collect();
        match matches.len() {
            0 => Err(format!("package not found in workspace: {package}")),
            1 => Ok(matches),
            _ => Err(format!("package name is ambiguous in workspace: {package}")),
        }
    } else {
        Ok(packages)
    }
}

fn load_workspace_packages(root: &Path) -> Result<Vec<Package>, String> {
    let manifest = load_manifest(&root.join("Cargo.toml"))?;
    let mut packages = Vec::new();
    if let Some(workspace) = manifest.workspace {
        for member in workspace.members {
            if member.contains('*') {
                return Err(format!(
                    "workspace member globs are not supported: {member}"
                ));
            }
            let package_path = PathBuf::from(&member);
            let member_manifest = load_manifest(&root.join(&package_path).join("Cargo.toml"))?;
            let package = member_manifest.package.ok_or_else(|| {
                format!(
                    "workspace member has no [package] section: {}",
                    path_to_string(&package_path)
                )
            })?;
            packages.push(Package {
                name: package.name,
                path: package_path,
            });
        }
    } else if let Some(package) = manifest.package {
        packages.push(Package {
            name: package.name,
            path: PathBuf::from("."),
        });
    } else {
        return Err("Cargo.toml contains neither [workspace] nor [package]".to_string());
    }
    packages.sort_by(|lhs, rhs| {
        lhs.path
            .cmp(&rhs.path)
            .then_with(|| lhs.name.cmp(&rhs.name))
    });
    Ok(packages)
}

fn load_manifest(path: &Path) -> Result<CargoManifest, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path_to_string(path)))?;
    toml::from_str(&contents)
        .map_err(|err| format!("failed to parse {}: {err}", path_to_string(path)))
}

fn audit_package(root: &Path, package: &Package) -> Result<Vec<MissingCounter>, String> {
    let package_root = root.join(&package.path);
    let src = package_root.join("src");
    let files = rust_files_under(&src)?;
    let mut counters = Vec::new();
    let mut registered = BTreeSet::new();
    for file in files {
        let relative_file = relative_to_root(root, &file);
        let source = fs::read_to_string(&file)
            .map_err(|err| format!("failed to read {}: {err}", path_to_string(&relative_file)))?;
        let audit = audit_rust_source(&package.path, &relative_file, &source)?;
        counters.extend(audit.counters);
        registered.extend(audit.registered);
    }
    Ok(missing_counters(counters, &registered))
}

fn rust_files_under(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)
        .map_err(|err| format!("failed to read directory {}: {err}", path_to_string(dir)))?
    {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read directory entry in {}: {err}",
                path_to_string(dir)
            )
        })?;
        entries.push(entry);
    }
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to stat {}: {err}", path_to_string(&path)))?;
        if file_type.is_dir() {
            if !is_ignored_dir(&path) {
                collect_rust_files(&path, files)?;
            }
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn is_ignored_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "target" || name.starts_with('.'))
}

fn audit_rust_source(package_path: &Path, file: &Path, source: &str) -> Result<FileAudit, String> {
    let parsed = syn::parse_file(source)
        .map_err(|err| format!("failed to parse {}: {err}", path_to_string(file)))?;
    Ok(audit_syntax_tree(package_path, file, &parsed))
}

fn audit_syntax_tree(package_path: &Path, file: &Path, parsed: &File) -> FileAudit {
    let mut visitor = CounterVisitor {
        package_path,
        file,
        function_depth: 0,
        audit: FileAudit::default(),
    };
    visitor.visit_file(parsed);
    visitor.audit
}

fn missing_counters(
    mut counters: Vec<CounterDefinition>,
    registered: &BTreeSet<String>,
) -> Vec<MissingCounter> {
    counters.sort_by(|lhs, rhs| {
        lhs.package_path
            .cmp(&rhs.package_path)
            .then_with(|| lhs.file.cmp(&rhs.file))
            .then_with(|| lhs.line.cmp(&rhs.line))
            .then_with(|| lhs.label.cmp(&rhs.label))
    });
    counters
        .into_iter()
        .filter(|counter| !registered.contains(&counter.ident))
        .map(|counter| MissingCounter {
            label: counter.label,
            package_path: counter.package_path,
            file: counter.file,
            line: counter.line,
        })
        .collect()
}

struct CounterVisitor<'a> {
    package_path: &'a Path,
    file: &'a Path,
    function_depth: usize,
    audit: FileAudit,
}

impl<'ast> Visit<'ast> for CounterVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_attrs(item).is_some_and(is_ignored_item) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.function_depth += 1;
        visit::visit_item_fn(self, item);
        self.function_depth -= 1;
    }

    fn visit_item_static(&mut self, item: &'ast ItemStatic) {
        if self.function_depth == 0 {
            if let Some(label) = counter_new_label(&item.expr) {
                self.audit.counters.push(CounterDefinition {
                    ident: item.ident.to_string(),
                    label,
                    package_path: self.package_path.to_path_buf(),
                    file: self.file.to_path_buf(),
                    line: item.ident.span().start().line,
                });
            }
        }
    }

    fn visit_expr_method_call(&mut self, method_call: &'ast ExprMethodCall) {
        if method_call.method == "register_counter" && method_call.args.len() == 1 {
            if let Some(ident) = registered_counter_ident(&method_call.args[0]) {
                self.audit.registered.insert(ident);
            }
        }
        visit::visit_expr_method_call(self, method_call);
    }
}

fn item_attrs(item: &Item) -> Option<&[Attribute]> {
    match item {
        Item::Const(item) => Some(&item.attrs),
        Item::Enum(item) => Some(&item.attrs),
        Item::ExternCrate(item) => Some(&item.attrs),
        Item::Fn(item) => Some(&item.attrs),
        Item::ForeignMod(item) => Some(&item.attrs),
        Item::Impl(item) => Some(&item.attrs),
        Item::Macro(item) => Some(&item.attrs),
        Item::Mod(item) => Some(&item.attrs),
        Item::Static(item) => Some(&item.attrs),
        Item::Struct(item) => Some(&item.attrs),
        Item::Trait(item) => Some(&item.attrs),
        Item::TraitAlias(item) => Some(&item.attrs),
        Item::Type(item) => Some(&item.attrs),
        Item::Union(item) => Some(&item.attrs),
        Item::Use(item) => Some(&item.attrs),
        Item::Verbatim(_) => None,
        _ => None,
    }
}

fn is_ignored_item(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_cfg_test_attr) || attrs.iter().any(is_test_attr)
}

fn is_test_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("test")
}

fn is_cfg_test_attr(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let mut requires_test = false;
    let _ = attr.parse_nested_meta(|meta| mark_cfg_requires_test(meta, &mut requires_test));
    requires_test
}

fn mark_cfg_requires_test(meta: ParseNestedMeta<'_>, requires_test: &mut bool) -> syn::Result<()> {
    if meta.path.is_ident("test") {
        *requires_test = true;
    } else if meta.path.is_ident("all") {
        meta.parse_nested_meta(|nested| mark_cfg_requires_test(nested, requires_test))?;
    }
    Ok(())
}

fn counter_new_label(expr: &Expr) -> Option<String> {
    let Expr::Call(call) = expr else {
        return None;
    };
    if call.args.len() != 1 || !is_counter_new_path_expr(&call.func) {
        return None;
    }
    let Expr::Lit(lit) = &call.args[0] else {
        return None;
    };
    let Lit::Str(label) = &lit.lit else {
        return None;
    };
    Some(label.value())
}

fn is_counter_new_path_expr(expr: &Expr) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    is_counter_new_path(&path.path)
}

fn is_counter_new_path(path: &SynPath) -> bool {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    segments == ["Counter", "new"] || segments == ["biometrics", "Counter", "new"]
}

fn registered_counter_ident(expr: &Expr) -> Option<String> {
    let Expr::Reference(reference) = expr else {
        return None;
    };
    let Expr::Path(path) = reference.expr.as_ref() else {
        return None;
    };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    Some(path.path.segments[0].ident.to_string())
}

fn relative_to_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn registered_static_counter_produces_no_missing_row() {
        let source = "use biometrics::{Collector, Counter};\nstatic REQUESTS: Counter = Counter::new(\"requests\");\nfn register(collector: &Collector) {\n    collector.register_counter(&REQUESTS);\n}\n";
        let missing = audit_sources(&[("pkg/src/lib.rs", source)]).unwrap();

        assert_eq!(Vec::<MissingCounter>::new(), missing);
    }

    #[test]
    fn unregistered_static_counter_reports_label_file_and_line() {
        let source =
            "use biometrics::Counter;\nstatic MISSED: Counter = Counter::new(\"missed\");\n";
        let missing = audit_sources(&[("pkg/src/lib.rs", source)]).unwrap();

        assert_eq!(
            vec![MissingCounter {
                label: "missed".to_string(),
                package_path: PathBuf::from("pkg"),
                file: PathBuf::from("pkg/src/lib.rs"),
                line: 2,
            }],
            missing
        );
    }

    #[test]
    fn counter_new_forms_are_detected() {
        let source = "use biometrics::Counter;\nstatic IMPORTED: Counter = Counter::new(\"imported\");\nstatic QUALIFIED: biometrics::Counter = biometrics::Counter::new(\"qualified\");\n";
        let missing = audit_sources(&[("pkg/src/lib.rs", source)]).unwrap();

        assert_eq!(
            vec![
                MissingCounter {
                    label: "imported".to_string(),
                    package_path: PathBuf::from("pkg"),
                    file: PathBuf::from("pkg/src/lib.rs"),
                    line: 2,
                },
                MissingCounter {
                    label: "qualified".to_string(),
                    package_path: PathBuf::from("pkg"),
                    file: PathBuf::from("pkg/src/lib.rs"),
                    line: 3,
                },
            ],
            missing
        );
    }

    #[test]
    fn test_only_and_local_counters_are_ignored() {
        let source = "use biometrics::Counter;\n#[cfg(test)]\nmod tests {\n    use super::*;\n    static TEST_ONLY: Counter = Counter::new(\"test.only\");\n}\n#[test]\nfn test_function() {\n    static LOCAL_TEST: Counter = Counter::new(\"local.test\");\n}\nfn runtime_function() {\n    static LOCAL_RUNTIME: Counter = Counter::new(\"local.runtime\");\n    let _counter = Counter::new(\"local.value\");\n}\n";
        let missing = audit_sources(&[("pkg/src/lib.rs", source)]).unwrap();

        assert_eq!(Vec::<MissingCounter>::new(), missing);
    }

    #[test]
    fn test_function_registration_does_not_register_production_counter() {
        let source = "use biometrics::{Collector, Counter};\nstatic PRODUCTION: Counter = Counter::new(\"production\");\n#[test]\nfn registers_only_in_tests() {\n    let collector = Collector::new();\n    collector.register_counter(&PRODUCTION);\n}\n";
        let missing = audit_sources(&[("pkg/src/lib.rs", source)]).unwrap();

        assert_eq!(
            vec![MissingCounter {
                label: "production".to_string(),
                package_path: PathBuf::from("pkg"),
                file: PathBuf::from("pkg/src/lib.rs"),
                line: 2,
            }],
            missing
        );
    }

    #[test]
    fn workspace_and_package_selection() {
        let temp = TempDir::new();
        temp.write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"one\", \"two\"]\nresolver = \"3\"\n",
        );
        temp.write(
            "one/Cargo.toml",
            "[package]\nname = \"one\"\nversion = \"0.0.0\"\n",
        );
        temp.write(
            "one/src/lib.rs",
            "static ONE: biometrics::Counter = biometrics::Counter::new(\"one.missing\");\n",
        );
        temp.write(
            "two/Cargo.toml",
            "[package]\nname = \"two\"\nversion = \"0.0.0\"\n",
        );
        temp.write(
            "two/src/lib.rs",
            "static TWO: biometrics::Counter = biometrics::Counter::new(\"two.missing\");\n",
        );

        let workspace_missing = audit_workspace(temp.path(), None).unwrap();
        let selected_missing = audit_workspace(temp.path(), Some("two")).unwrap();

        assert_eq!(
            vec![
                MissingCounter {
                    label: "one.missing".to_string(),
                    package_path: PathBuf::from("one"),
                    file: PathBuf::from("one/src/lib.rs"),
                    line: 1,
                },
                MissingCounter {
                    label: "two.missing".to_string(),
                    package_path: PathBuf::from("two"),
                    file: PathBuf::from("two/src/lib.rs"),
                    line: 1,
                },
            ],
            workspace_missing
        );
        assert_eq!(
            vec![MissingCounter {
                label: "two.missing".to_string(),
                package_path: PathBuf::from("two"),
                file: PathBuf::from("two/src/lib.rs"),
                line: 1,
            }],
            selected_missing
        );
    }

    fn audit_sources(sources: &[(&str, &str)]) -> Result<Vec<MissingCounter>, String> {
        let package_path = Path::new("pkg");
        let mut counters = Vec::new();
        let mut registered = BTreeSet::new();
        for (file, source) in sources {
            let audit = audit_rust_source(package_path, Path::new(file), source)?;
            counters.extend(audit.counters);
            registered.extend(audit.registered);
        }
        Ok(missing_counters(counters, &registered))
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let nonce = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "biometrics-audit-counters-test-{}-{nanos}-{nonce}",
                process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
