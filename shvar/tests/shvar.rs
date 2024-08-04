// AUTO-GENERATED FILE:  DO NOT EDIT MANUALLY (or with sed)
// regenerate with:  cargo run --example tests > tests/shvar.rs && cargo fmt
use shvar::expand;
use std::collections::HashMap;

#[test]
fn foo_notset_1() {
    let env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&env, "${FOO}").unwrap())
}

#[test]
fn foo_isset_1() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&env, "${FOO}").unwrap())
}

#[test]
fn foo_notset_default_bar_notset_2() {
    let env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_2() {
    let env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_2() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_2() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_2() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("bar", expand(&env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_2() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_2() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_2() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("bar", expand(&env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("bar", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_default_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("bar", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_alternate_baz_notset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("baz", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("baz", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("bar", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("baz", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_default_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("bar", expand(&env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_alternate_baz_isset_3() {
    let env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("baz", expand(&env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}
