use utf8path::Path;

const VARIABLES: &[(&str, &str)] = &[
    ("SALUTATION", "Hello Alice!"),
    ("foo1_ENABLED", "YES"),
    ("foo1_ENABLED", "NO"),
    ("foo1_SALUTATION", "Hej Bob!"),
    ("bar7_SALUTATION", "Привет Charlie"),
];

const FILES: &[(&str, &str)] = &[
    (
        "templates/service.yaml.template",
        "source: this is the generic template
salutation: ${SALUTATION:-<UNSET>}",
    ),
    (
        "templates/rc.d/foo1.yaml.template",
        "source: this is foo1 template
salutation: ${SALUTATION:-<UNSET>}",
    ),
    (
        "templates/rc.d/bar7.yaml.template",
        "source: this is bar7 template
salutation: ${SALUTATION:-<UNSET>}",
    ),
];

fn n_choose_k(k: usize, n: usize) -> impl Iterator<Item = usize> {
    assert!(n < 64);
    assert!(k <= n);
    struct Count {
        k: usize,
        n: usize,
        i: usize,
    }
    impl Iterator for Count {
        type Item = usize;

        fn next(&mut self) -> Option<Self::Item> {
            while self.i < 1 << self.n {
                let i = self.i;
                self.i += 1;
                if i.count_ones() as usize <= self.k {
                    return Some(i);
                }
            }
            None
        }
    }
    Count { k, n, i: 0 }
}

fn generate_combinations<'a, 'b>(
    inputs: &'a [(&'b str, &'b str)],
    k: usize,
) -> impl Iterator<Item = Vec<(&'b str, &'b str)>> + 'a {
    assert!(k <= inputs.len());
    struct Combinate<'b, 'c, I: Iterator<Item = usize>> {
        combinations: I,
        inputs: &'b [(&'c str, &'c str)],
    }
    impl<'c, I: Iterator<Item = usize>> Iterator for Combinate<'_, 'c, I> {
        type Item = Vec<(&'c str, &'c str)>;

        fn next(&mut self) -> Option<Self::Item> {
            'combinating: for mask in self.combinations.by_ref() {
                for i in 0..self.inputs.len() {
                    for j in i + 1..self.inputs.len() {
                        if mask & (1 << i) != 0
                            && mask & (1 << j) != 0
                            && self.inputs[i].0 == self.inputs[j].0
                        {
                            continue 'combinating;
                        }
                    }
                }
                let mut result = Vec::with_capacity(mask.count_ones() as usize);
                for i in 0..self.inputs.len() {
                    if mask & (1 << i) != 0 {
                        result.push(self.inputs[i]);
                    }
                }
                return Some(result);
            }
            None
        }
    }
    Combinate {
        combinations: n_choose_k(k, inputs.len()),
        inputs,
    }
}

fn main() {
    let mut enabled_rc_confs = vec![];
    for vars in generate_combinations(VARIABLES, 3) {
        let mut rc_conf = r#"NAMESPACE="symphhonize"
foo1_IMAGE="foo1:latest"
foo1_PORT="6600"
"#
        .to_string();
        let mut enabled = false;
        for (key, value) in vars {
            if key == "foo1_ENABLED" && value == "YES" {
                enabled = true;
            }
            rc_conf += &format!("{key}={value:?}\n")
        }
        if enabled {
            enabled_rc_confs.push(rc_conf);
        }
    }
    let mut files = FILES.to_vec();
    files.extend(enabled_rc_confs.iter().map(|x| ("rc.conf", x.as_str())));
    let mut idx = 0;
    for files in generate_combinations(&files, 4) {
        let case_path = Path::from(format!("tests/cases/{idx}"));
        if case_path.exists() {
            std::fs::remove_dir_all(&case_path).expect("should be able to remove directory");
        }
        if !files.iter().any(|f| f.0 == "rc.conf") {
            continue;
        }
        for (file, contents) in files {
            let file = case_path.join(file);
            std::fs::create_dir_all(file.dirname()).expect("create dir should succeed");
            std::fs::write(file, contents).expect("writing file should succeed");
        }
        idx += 1;
    }
}
