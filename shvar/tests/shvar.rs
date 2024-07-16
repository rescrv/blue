// AUTO-GENERATED FILE:  DO NOT EDIT MANUALLY (or with sed)
// regenerate with:  cargo run --example tests > tests/shvar.rs && cargo fmt
use shvar::{expand, quote};
use std::collections::HashMap;

#[test]
fn foo_notset_1() {
    let mut env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&mut env, "${FOO}").unwrap())
}

#[test]
fn foo_isset_1() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&mut env, "${FOO}").unwrap())
}

#[test]
fn foo_notset_default_bar_notset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&mut env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("bar", expand(&mut env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_2() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("bar", expand(&mut env, "${FOO:+${BAR}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("bar", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_default_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("bar", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_alternate_baz_notset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAR", "bar")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("baz", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_notset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_notset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAZ", "baz")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_notset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("baz", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_notset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("FOO", "foo"), ("BAZ", "baz")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("bar", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_default_bar_isset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("baz", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_notset_alternate_bar_isset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> = HashMap::from([("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> =
        HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_default_bar_isset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> =
        HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("foo", expand(&mut env, "${FOO:-${BAR:+${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_default_baz_isset_3() {
    let mut env: HashMap<&str, &str> =
        HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("bar", expand(&mut env, "${FOO:+${BAR:-${BAZ}}}").unwrap());
}

#[test]
fn foo_isset_alternate_bar_isset_alternate_baz_isset_3() {
    let mut env: HashMap<&str, &str> =
        HashMap::from([("FOO", "foo"), ("BAR", "bar"), ("BAZ", "baz")]);
    assert_eq!("baz", expand(&mut env, "${FOO:+${BAR:+${BAZ}}}").unwrap());
}
