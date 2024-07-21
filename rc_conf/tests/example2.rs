use std::process::{Command, Stdio};

mod common;

use common::cargo_dir;

#[test]
fn example2() {
    let cd = cargo_dir();
    let rcscript = cd.into_std().join("rcscript");
    let mut output = Command::new(rcscript)
        .arg("rc.d/example2")
        .arg("rcvar")
        .stdout(Stdio::piped())
        .output()
        .expect("rcscript should spawn");
    assert_eq!(0, output.status.code().unwrap_or(0));
    let s = String::from_utf8(output.stdout);
    assert_eq!("example2_FIELD1\nexample2_FIELD2\n", s.unwrap());
}
