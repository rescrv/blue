use utf8path::Path;

use k8src::{
    RegenerateOptions, candidates, error_code, error_message, error_string_field, regenerate,
    template_resolution,
};
use rc_conf::RcConf;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_CASE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct TempCase {
    root: Path<'static>,
}

impl TempCase {
    fn new() -> Self {
        let id = TEMP_CASE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("k8src_test_{}_{}", std::process::id(), id));
        std::fs::create_dir_all(&path).expect("temp case directory should be writable");
        let root = Path::try_from(path).expect("temp case path should be UTF-8");
        Self { root }
    }

    fn write_rc_conf(&self, contents: &str) {
        std::fs::write(self.root.join("rc.conf"), contents).expect("rc.conf should be writable");
    }
}

impl Drop for TempCase {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

macro_rules! test_case {
    ($name:ident, $num:literal) => {
        #[test]
        fn $name() {
            let root = Path::from(format!("tests/cases/{}", $num));
            let output = root.join("manifests");
            test_case(root, output);
        }
    };
}

fn temp_root(tag: &str) -> Path<'static> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = Path::from(format!("/tmp/k8src-regression-tests-{tag}-{nanos}"));
    std::fs::create_dir_all(root.as_str()).expect("should create temporary root");
    root
}

fn cleanup_root(root: &Path) {
    let _ = std::fs::remove_dir_all(root.as_str());
}

fn write(path: Path, contents: &str) {
    std::fs::create_dir_all(path.dirname().as_str()).expect("should create parent directory");
    std::fs::write(path.as_str(), contents).expect("should write file");
}

fn basic_rc_conf() -> &'static str {
    r#"NAMESPACE="default"
foo_ENABLED="YES"
foo_IMAGE="foo:latest"
foo_PORT="8080"
"#
}

fn k8src_bin() -> &'static str {
    env!("CARGO_BIN_EXE_k8src")
}

fn test_case(path: Path, output: Path) {
    let options = RegenerateOptions {
        root: Some(path.as_str().to_string()),
        output: Some(output.as_str().to_string()),
        verify: true,
        overwrite: false,
        dry_run: false,
        diff: false,
    };
    regenerate(options).expect("regenerate should never fail");
}

#[test]
fn missing_image_returns_error() {
    let case = TempCase::new();
    case.write_rc_conf(
        r#"
NAMESPACE="k8src-test"
IMAGE_RCVAR=""
svc_ENABLED="YES"
svc_PORT="1234"
"#,
    );
    let options = RegenerateOptions {
        root: Some(case.root.as_str().to_string()),
        output: Some(case.root.join("manifests").as_str().to_string()),
        verify: false,
        overwrite: false,
        dry_run: false,
        diff: false,
    };
    let err = regenerate(options).expect_err("expected missing IMAGE");
    assert_eq!(error_code(&err), Some("missing-required-variable"));
    assert_eq!(error_string_field(&err, "service"), Some("svc".to_string()));
    assert_eq!(error_string_field(&err, "key"), Some("IMAGE".to_string()));
}

test_case!(case0, 0);
test_case!(case1, 1);
test_case!(case2, 2);
test_case!(case3, 3);
test_case!(case4, 4);
test_case!(case5, 5);
test_case!(case6, 6);
test_case!(case7, 7);
test_case!(case8, 8);
test_case!(case9, 9);
test_case!(case10, 10);
test_case!(case11, 11);
test_case!(case12, 12);
test_case!(case13, 13);
test_case!(case14, 14);
test_case!(case15, 15);
test_case!(case16, 16);
test_case!(case17, 17);
test_case!(case18, 18);
test_case!(case19, 19);
test_case!(case20, 20);
test_case!(case21, 21);
test_case!(case22, 22);
test_case!(case23, 23);
test_case!(case24, 24);
test_case!(case25, 25);
test_case!(case26, 26);
test_case!(case27, 27);
test_case!(case28, 28);
test_case!(case29, 29);
test_case!(case30, 30);
test_case!(case31, 31);
test_case!(case32, 32);
test_case!(case33, 33);
test_case!(case34, 34);
test_case!(case35, 35);
test_case!(case36, 36);
test_case!(case37, 37);
test_case!(case38, 38);
test_case!(case39, 39);
test_case!(case40, 40);
test_case!(case41, 41);
test_case!(case42, 42);
test_case!(case43, 43);
test_case!(case44, 44);
test_case!(case45, 45);
test_case!(case46, 46);
test_case!(case47, 47);
test_case!(case48, 48);
test_case!(case49, 49);
test_case!(case50, 50);
test_case!(case51, 51);
test_case!(case52, 52);
test_case!(case53, 53);

test_case!(case99, 99);

#[test]
fn missing_required_image_variable_is_reported() {
    let root = temp_root("missing-image");
    std::fs::write(
        root.join("rc.conf").as_str(),
        r#"foo_ENABLED="YES"
foo_PORT="8080"
"#,
    )
    .expect("should write temporary rc.conf");
    let err = regenerate(RegenerateOptions {
        root: Some(root.as_str().to_string()),
        output: Some(root.join("manifests").as_str().to_string()),
        verify: false,
        overwrite: true,
        dry_run: false,
        diff: false,
    })
    .expect_err("should fail on missing IMAGE");
    assert_eq!(error_code(&err), Some("missing-required-variable"));
    assert_eq!(error_string_field(&err, "service"), Some("foo".to_string()));
    assert_eq!(error_string_field(&err, "key"), Some("IMAGE".to_string()));
    cleanup_root(&root);
}

#[test]
fn deepest_overlay_template_wins() {
    let root = temp_root("deepest-overlay-template");
    write(root.join("rc.conf"), basic_rc_conf());
    write(
        root.join("service.yaml.template"),
        "source: root\nimage: ${IMAGE:?}\n",
    );
    write(root.join("env/rc.conf"), "");
    write(
        root.join("env/service.yaml.template"),
        "source: env\nimage: ${IMAGE:?}\n",
    );
    let candidates = candidates(&root, &root.join("env/rc.conf")).expect("candidates");
    let rc_conf = RcConf::parse(&k8src::rc_conf_path(&candidates)).expect("rc.conf");
    let resolution = template_resolution(&candidates, &rc_conf, "foo");
    assert_eq!(
        resolution.selected,
        Some(root.join("env/service.yaml.template").as_str().to_string())
    );
    cleanup_root(&root);
}

#[test]
fn service_specific_template_wins_over_default() {
    let root = temp_root("service-specific-template");
    write(root.join("rc.conf"), basic_rc_conf());
    write(root.join("service.yaml.template"), "source: default\n");
    write(root.join("rc.d/foo.yaml.template"), "source: foo\n");
    let candidates = candidates(&root, &root.join("rc.conf")).expect("candidates");
    let rc_conf = RcConf::parse(&k8src::rc_conf_path(&candidates)).expect("rc.conf");
    let resolution = template_resolution(&candidates, &rc_conf, "foo");
    assert_eq!(
        resolution.selected,
        Some(root.join("rc.d/foo.yaml.template").as_str().to_string())
    );
    cleanup_root(&root);
}

#[test]
fn alias_fallback_resolves_transitively() {
    let root = temp_root("alias-template");
    write(
        root.join("rc.conf"),
        r#"NAMESPACE="default"
base_ENABLED="YES"
base_IMAGE="base:latest"
base_PORT="8080"
mid_ENABLED="YES"
mid_ALIASES="base"
leaf_ENABLED="YES"
leaf_ALIASES="mid"
"#,
    );
    write(root.join("rc.d/base.yaml.template"), "source: base\n");
    let candidates = candidates(&root, &root.join("rc.conf")).expect("candidates");
    let rc_conf = RcConf::parse(&k8src::rc_conf_path(&candidates)).expect("rc.conf");
    let resolution = template_resolution(&candidates, &rc_conf, "leaf");
    assert_eq!(
        resolution.selected,
        Some(root.join("rc.d/base.yaml.template").as_str().to_string())
    );
    cleanup_root(&root);
}

#[test]
fn built_in_default_is_used_when_no_template_exists() {
    let root = temp_root("builtin-template");
    write(root.join("rc.conf"), basic_rc_conf());
    let candidates = candidates(&root, &root.join("rc.conf")).expect("candidates");
    let rc_conf = RcConf::parse(&k8src::rc_conf_path(&candidates)).expect("rc.conf");
    let resolution = template_resolution(&candidates, &rc_conf, "foo");
    assert_eq!(resolution.selected, None);
    assert!(resolution.uses_builtin_default);
    regenerate(RegenerateOptions {
        root: Some(root.as_str().to_string()),
        output: Some(root.join("manifests").as_str().to_string()),
        verify: false,
        overwrite: true,
        dry_run: false,
        diff: false,
    })
    .expect("regenerate should use built-in template");
    let manifest = std::fs::read_to_string(root.join("manifests/herd/foo.yaml").as_str())
        .expect("manifest should exist");
    assert!(manifest.contains("kind: Deployment"));
    cleanup_root(&root);
}

#[test]
fn dry_run_does_not_write_manifests() {
    let root = temp_root("dry-run");
    write(root.join("rc.conf"), basic_rc_conf());
    regenerate(RegenerateOptions {
        root: Some(root.as_str().to_string()),
        output: Some(root.join("manifests").as_str().to_string()),
        verify: false,
        overwrite: false,
        dry_run: true,
        diff: false,
    })
    .expect("dry-run should succeed");
    assert!(
        !root
            .join("manifests")
            .exists()
            .expect("should inspect manifests")
    );
    cleanup_root(&root);
}

#[test]
fn diff_does_not_write_manifests() {
    let root = temp_root("diff");
    write(root.join("rc.conf"), basic_rc_conf());
    regenerate(RegenerateOptions {
        root: Some(root.as_str().to_string()),
        output: Some(root.join("manifests").as_str().to_string()),
        verify: false,
        overwrite: false,
        dry_run: false,
        diff: true,
    })
    .expect("diff should succeed");
    assert!(
        !root
            .join("manifests")
            .exists()
            .expect("should inspect manifests")
    );
    cleanup_root(&root);
}

#[test]
fn explain_template_reports_selected_template() {
    let root = temp_root("explain-template");
    write(root.join("rc.conf"), basic_rc_conf());
    write(root.join("rc.d/foo.yaml.template"), "source: foo\n");
    let explanation = k8src::explain_template(Some(root.as_str()), "foo").expect("explain");
    assert!(explanation.contains("selected: "));
    assert!(explanation.contains("rc.d/foo.yaml.template"));
    assert!(explanation.contains("fallback_chain:"));
    cleanup_root(&root);
}

#[test]
fn explain_vars_reports_effective_values() {
    let root = temp_root("explain-vars");
    write(root.join("rc.conf"), basic_rc_conf());
    let explanation = k8src::explain_vars(Some(root.as_str()), "foo").expect("explain");
    assert!(explanation.contains("IMAGE=foo:latest"));
    assert!(explanation.contains("PORT=8080"));
    assert!(explanation.contains("NAMESPACE=default"));
    cleanup_root(&root);
}

#[test]
fn cli_template_prints_built_in_default() {
    let output = Command::new(k8src_bin())
        .args(["template", "service.yaml.template"])
        .output()
        .expect("should run k8src");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("kind: Deployment"));
    assert!(stdout.contains("configMapRef"));
}

#[test]
fn cli_init_creates_runnable_skeleton() {
    let root = temp_root("cli-init");
    let output = Command::new(k8src_bin())
        .args(["init", root.as_str()])
        .output()
        .expect("should run k8src");
    assert!(output.status.success());
    assert!(
        root.join("rc.conf")
            .exists()
            .expect("should inspect rc.conf")
    );
    assert!(
        root.join("service.yaml.template")
            .exists()
            .expect("should inspect service template")
    );
    assert!(root.join("rc.d").exists().expect("should inspect rc.d"));
    assert!(root.join("pets").exists().expect("should inspect pets"));
    assert!(
        root.join(".k8srcignore")
            .exists()
            .expect("should inspect .k8srcignore")
    );
    let dry_run = Command::new(k8src_bin())
        .args(["regenerate", "--root", root.as_str(), "--dry-run"])
        .output()
        .expect("should run k8src");
    assert!(dry_run.status.success());
    let stdout = String::from_utf8(dry_run.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("herd/example.yaml"));
    cleanup_root(&root);
}

#[test]
fn cli_dry_run_prints_generated_paths_without_writing() {
    let root = temp_root("cli-dry-run");
    write(root.join("rc.conf"), basic_rc_conf());
    let output = Command::new(k8src_bin())
        .args([
            "regenerate",
            "--root",
            root.as_str(),
            "--output",
            root.join("manifests").as_str(),
            "--dry-run",
        ])
        .output()
        .expect("should run k8src");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("would generate"));
    assert!(stdout.contains("herd/foo.yaml"));
    assert!(
        !root
            .join("manifests")
            .exists()
            .expect("should inspect manifests")
    );
    cleanup_root(&root);
}

#[test]
fn cli_diff_prints_unified_diff_without_writing() {
    let root = temp_root("cli-diff");
    write(root.join("rc.conf"), basic_rc_conf());
    let output = Command::new(k8src_bin())
        .args([
            "regenerate",
            "--root",
            root.as_str(),
            "--output",
            root.join("manifests").as_str(),
            "--diff",
        ])
        .output()
        .expect("should run k8src");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("--- /dev/null"));
    assert!(stdout.contains("+++ "));
    assert!(stdout.contains("+kind: Deployment"));
    assert!(
        !root
            .join("manifests")
            .exists()
            .expect("should inspect manifests")
    );
    cleanup_root(&root);
}

#[test]
fn cli_missing_variable_error_is_readable() {
    let root = temp_root("cli-missing-variable");
    write(
        root.join("rc.conf"),
        r#"foo_ENABLED="YES"
foo_PORT="8080"
"#,
    );
    let output = Command::new(k8src_bin())
        .args([
            "regenerate",
            "--root",
            root.as_str(),
            "--output",
            root.join("manifests").as_str(),
            "--overwrite",
        ])
        .output()
        .expect("should run k8src");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("IMAGE is required"));
    assert!(stderr.contains("service: foo"));
    assert!(stderr.contains("rc_conf_path:"));
    cleanup_root(&root);
}

#[test]
fn cli_explain_commands_print_details() {
    let root = temp_root("cli-explain");
    write(root.join("rc.conf"), basic_rc_conf());
    write(root.join("rc.d/foo.yaml.template"), "source: foo\n");
    let template = Command::new(k8src_bin())
        .args(["explain-template", "--root", root.as_str(), "foo"])
        .output()
        .expect("should run k8src");
    assert!(template.status.success());
    let stdout = String::from_utf8(template.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("selected:"));
    assert!(stdout.contains("rc.d/foo.yaml.template"));
    let vars = Command::new(k8src_bin())
        .args(["explain-vars", "--root", root.as_str(), "foo"])
        .output()
        .expect("should run k8src");
    assert!(vars.status.success());
    let stdout = String::from_utf8(vars.stdout).expect("stdout should be utf8");
    assert!(stdout.contains("IMAGE=foo:latest"));
    cleanup_root(&root);
}

#[test]
fn candidates_rejects_rc_conf_outside_root() {
    let root = temp_root("candidates");
    let outside = Path::from(format!(
        "/tmp/k8src-regression-outside-{}",
        root.as_str().len()
    ));
    let err = candidates(&root, &outside).expect_err("should reject rc_conf outside root");
    assert_eq!(error_code(&err), Some("invalid-root"));
    assert_eq!(
        error_string_field(&err, "path"),
        Some(root.as_str().to_string())
    );
    assert_eq!(
        error_string_field(&err, "rc_conf_path"),
        Some(outside.as_str().to_string())
    );
    cleanup_root(&root);
}

#[test]
fn image_rcvar_command_failure_captures_context() {
    let root = temp_root("image-rcvar-failure");
    std::fs::write(
        root.join("rc.conf").as_str(),
        r#"foo_ENABLED="YES"
foo_IMAGE="foo:latest"
foo_PORT="8080"
foo_NAMESPACE="default"
foo_IMAGE_RCVAR=/usr/bin/false
"#,
    )
    .expect("should write temporary rc.conf");
    let err = regenerate(RegenerateOptions {
        root: Some(root.as_str().to_string()),
        output: Some(root.join("manifests").as_str().to_string()),
        verify: false,
        overwrite: true,
        dry_run: false,
        diff: false,
    })
    .expect_err("should report IMAGE_RCVAR command failure");
    assert_eq!(error_code(&err), Some("command-failed"));
    assert_eq!(error_string_field(&err, "service"), Some("foo".to_string()));
    assert_eq!(
        error_string_field(&err, "rc_command"),
        Some("/usr/bin/false".to_string())
    );
    assert_eq!(
        error_string_field(&err, "context"),
        Some("running IMAGE_RCVAR command".to_string())
    );
    cleanup_root(&root);
}

#[cfg(unix)]
#[test]
fn non_utf8_directory_entry_triggers_non_utf8_path_error() {
    use std::ffi::OsString;
    let root = temp_root("non-utf8-path");
    std::fs::write(
        root.join("rc.conf").as_str(),
        r#"foo_ENABLED="YES"
foo_IMAGE="foo:latest"
foo_PORT="8080"
"#,
    )
    .expect("should write temporary rc.conf");
    let bad_name = OsString::from_vec(vec![0xff]);
    let bad_dir = std::path::Path::new(root.as_str()).join(bad_name);
    if std::fs::create_dir_all(&bad_dir).is_err() {
        cleanup_root(&root);
        return;
    }
    let err = regenerate(RegenerateOptions {
        root: Some(root.as_str().to_string()),
        output: Some(root.join("manifests").as_str().to_string()),
        verify: false,
        overwrite: true,
        dry_run: false,
        diff: false,
    })
    .expect_err("should report non-UTF8 directory entry path");
    assert_eq!(error_code(&err), Some("non-utf8-path"));
    assert_eq!(
        error_message(&err),
        Some("non-UTF8 directory entry path encountered".to_string())
    );
    cleanup_root(&root);
}
