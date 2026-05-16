use utf8path::Path;

use k8src::{RegenerateOptions, regenerate};

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

fn test_case(path: Path, output: Path) {
    let options = RegenerateOptions {
        root: Some(path.as_str().to_string()),
        output: Some(output.as_str().to_string()),
        verify: true,
        overwrite: false,
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
    };
    match regenerate(options) {
        Err(k8src::Error::MissingImage { service }) => assert_eq!("svc", service),
        Err(err) => panic!("expected MissingImage; got {err:?}"),
        Ok(()) => panic!("expected MissingImage; got Ok"),
    }
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
