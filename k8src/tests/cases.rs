use utf8path::Path;

use k8src::{regenerate, RegenerateOptions};

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
