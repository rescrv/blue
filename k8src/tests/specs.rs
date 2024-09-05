use yaml_rust::{Yaml, YamlEmitter, YamlLoader};

use rc_conf::RcConf;
use utf8path::Path;

fn yaml_to_string(yaml: &Yaml) -> String {
    let mut out = String::new();
    let mut emitter = YamlEmitter::new(&mut out);
    emitter.dump(yaml).expect("emitter should emit");
    out
}

fn run_test(spec: &Path, idx: usize, doc: Yaml) {
    if let Yaml::Hash(h) = doc {
        let rc_conf = h
            .get(&Yaml::String("rc_conf".to_string()))
            .expect("there should be an rc_conf field");
        let Yaml::String(rc_conf) = rc_conf else {
            panic!("rc_conf must be a string");
        };
        let tempfile = format!("{spec}.{idx}.rc.conf");
        std::fs::write(&tempfile, rc_conf).expect("should be able to write tempfile");
        let rc_conf = RcConf::parse(&tempfile).expect("should be able to parse rc.conf");
        let rc_d = h
            .get(&Yaml::String("rc_d".to_string()))
            .expect("there should be an rc_d field");
        let Yaml::String(rc_d) = rc_d else {
            panic!("rc_d must be a string");
        };
        let template = h
            .get(&Yaml::String("template".to_string()))
            .expect("there should be an template field");
        if let Some(expected) = h.get(&Yaml::String("expected".to_string())) {
            let returned = k8src::rewrite(&rc_conf, rc_d, &yaml_to_string(template))
                .expect("the rewrite pass should not fail");
            assert_eq!(
                yaml_to_string(expected).trim(),
                returned.trim(),
                "{spec}[{idx}]"
            );
            println!("success: {tempfile}");
        } else if let Some(Yaml::String(error)) = h.get(&Yaml::String("error".to_string())) {
            match k8src::rewrite(&rc_conf, rc_d, &yaml_to_string(template)) {
                Ok(_) => panic!("rewrite succeeded, but error was expected {spec}[{idx}]"),
                Err(k8src::Error::Shvar(shvar::Error::Requested(message))) => {
                    assert_eq!(*error, message);
                }
                Err(err) => panic!("unhandled error: {err:?}"),
            }
        } else {
            panic!("unhandled case");
        }
    } else {
        panic!("top level object must be a hash; got:\n{doc:?}");
    }
}

fn main() {
    for spec in std::fs::read_dir("tests").expect("should be able to read dir") {
        let spec = spec.expect("should be ablle to read dirent");
        let spec = Path::try_from(spec.path()).expect("spec should be utf8 path name");
        if !spec.as_str().ends_with(".yaml.spec") {
            continue;
        }
        let yaml = std::fs::read_to_string(&spec).expect("should be able to read");
        let docs = YamlLoader::load_from_str(&yaml).expect("should be able to parse yaml");
        for (idx, doc) in docs.into_iter().enumerate() {
            run_test(&spec, idx, doc);
        }
    }
}
