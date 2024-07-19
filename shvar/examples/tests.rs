#![allow(clippy::disallowed_names)]

const ANSWERS: &[(&str, &str)] = &[
    ("foo_isset", "foo"),
    ("foo_isset_alternate_bar_isset", "bar"),
    ("foo_isset_alternate_bar_isset_alternate_baz_isset", "baz"),
    ("foo_isset_alternate_bar_isset_alternate_baz_notset", ""),
    ("foo_isset_alternate_bar_isset_default_baz_isset", "bar"),
    ("foo_isset_alternate_bar_isset_default_baz_notset", "bar"),
    ("foo_isset_alternate_bar_notset", ""),
    ("foo_isset_alternate_bar_notset_alternate_baz_isset", ""),
    ("foo_isset_alternate_bar_notset_alternate_baz_notset", ""),
    ("foo_isset_alternate_bar_notset_default_baz_isset", "baz"),
    ("foo_isset_alternate_bar_notset_default_baz_notset", ""),
    ("foo_isset_default_bar_isset", "foo"),
    ("foo_isset_default_bar_isset_alternate_baz_isset", "foo"),
    ("foo_isset_default_bar_isset_alternate_baz_notset", "foo"),
    ("foo_isset_default_bar_isset_default_baz_isset", "foo"),
    ("foo_isset_default_bar_isset_default_baz_notset", "foo"),
    ("foo_isset_default_bar_notset", "foo"),
    ("foo_isset_default_bar_notset_alternate_baz_isset", "foo"),
    ("foo_isset_default_bar_notset_alternate_baz_notset", "foo"),
    ("foo_isset_default_bar_notset_default_baz_isset", "foo"),
    ("foo_isset_default_bar_notset_default_baz_notset", "foo"),
    ("foo_notset", ""),
    ("foo_notset_alternate_bar_isset", ""),
    ("foo_notset_alternate_bar_isset_alternate_baz_isset", ""),
    ("foo_notset_alternate_bar_isset_alternate_baz_notset", ""),
    ("foo_notset_alternate_bar_isset_default_baz_isset", ""),
    ("foo_notset_alternate_bar_isset_default_baz_notset", ""),
    ("foo_notset_alternate_bar_notset", ""),
    ("foo_notset_alternate_bar_notset_alternate_baz_isset", ""),
    ("foo_notset_alternate_bar_notset_alternate_baz_notset", ""),
    ("foo_notset_alternate_bar_notset_default_baz_isset", ""),
    ("foo_notset_alternate_bar_notset_default_baz_notset", ""),
    ("foo_notset_default_bar_isset", "bar"),
    ("foo_notset_default_bar_isset_alternate_baz_isset", "baz"),
    ("foo_notset_default_bar_isset_alternate_baz_notset", ""),
    ("foo_notset_default_bar_isset_default_baz_isset", "bar"),
    ("foo_notset_default_bar_isset_default_baz_notset", "bar"),
    ("foo_notset_default_bar_notset", ""),
    ("foo_notset_default_bar_notset_alternate_baz_isset", ""),
    ("foo_notset_default_bar_notset_alternate_baz_notset", ""),
    ("foo_notset_default_bar_notset_default_baz_isset", "baz"),
    ("foo_notset_default_bar_notset_default_baz_notset", ""),
];

#[derive(Clone, Copy, Debug)]
enum Operator {
    DefaultValue,
    AlternateValue,
}

impl Operator {
    fn as_str(self) -> &'static str {
        match self {
            Self::DefaultValue => "-",
            Self::AlternateValue => "+",
        }
    }
}

const OPS: &[Operator] = &[Operator::DefaultValue, Operator::AlternateValue];

fn expect(test_name: &str) -> &str {
    for (tn, e) in ANSWERS.iter() {
        if *tn == test_name {
            return e;
        }
    }
    ""
}

fn generate_test_case1(foo: Option<String>) {
    let mut test_name = String::new();
    if foo.is_some() {
        test_name += "foo_isset";
    } else {
        test_name += "foo_notset";
    };

    println!("\n#[test]");
    println!("fn {test_name}_1() {{");

    println!("    let mut env: HashMap<&str, &str> = HashMap::from([");
    if let Some(foo) = foo {
        println!("        (\"FOO\", {foo:?}),");
    }
    println!("    ]);");

    let expect = expect(&test_name);
    let mut assertion = format!("assert_eq!({expect:?}");
    assertion += ", expand(&mut env, \"${FOO}\").unwrap())";
    println!("    {assertion}");
    println!("}}");
}

fn generate_test_case3(foo: Option<String>, op1: Operator, bar: Option<String>) {
    let mut test_name = String::new();
    if foo.is_some() {
        test_name += "foo_isset_";
    } else {
        test_name += "foo_notset_";
    };
    test_name += match op1 {
        Operator::DefaultValue => "default",
        Operator::AlternateValue => "alternate",
    };
    if bar.is_some() {
        test_name += "_bar_isset";
    } else {
        test_name += "_bar_notset";
    };

    println!("\n#[test]");
    println!("fn {test_name}_2() {{");

    println!("    let mut env: HashMap<&str, &str> = HashMap::from([");
    if let Some(foo) = foo {
        println!("        (\"FOO\", {foo:?}),");
    }
    if let Some(bar) = bar {
        println!("        (\"BAR\", {bar:?}),");
    }
    println!("    ]);");
    let expect = expect(&test_name);
    let mut assertion = format!("assert_eq!({expect:?}");
    assertion += ", expand(&mut env, \"${FOO:";
    assertion += op1.as_str();
    assertion += "${BAR}}\").unwrap());";
    println!("    {assertion}");
    println!("}}");
}

fn generate_test_case5(
    foo: Option<String>,
    op1: Operator,
    bar: Option<String>,
    op2: Operator,
    baz: Option<String>,
) {
    let mut test_name = String::new();
    if foo.is_some() {
        test_name += "foo_isset_";
    } else {
        test_name += "foo_notset_";
    };
    test_name += match op1 {
        Operator::DefaultValue => "default",
        Operator::AlternateValue => "alternate",
    };
    if bar.is_some() {
        test_name += "_bar_isset_";
    } else {
        test_name += "_bar_notset_";
    };
    test_name += match op2 {
        Operator::DefaultValue => "default",
        Operator::AlternateValue => "alternate",
    };
    if baz.is_some() {
        test_name += "_baz_isset";
    } else {
        test_name += "_baz_notset";
    };

    println!("\n#[test]");
    println!("fn {test_name}_3() {{");

    println!("    let mut env: HashMap<&str, &str> = HashMap::from([");
    if let Some(foo) = foo {
        println!("        (\"FOO\", {foo:?}),");
    }
    if let Some(bar) = bar {
        println!("        (\"BAR\", {bar:?}),");
    }
    if let Some(baz) = baz {
        println!("        (\"BAZ\", {baz:?}),");
    }
    println!("    ]);");
    let expect = expect(&test_name);
    let mut assertion = format!("assert_eq!({expect:?}");
    assertion += ", expand(&mut env, \"${FOO:";
    assertion += op1.as_str();
    assertion += "${BAR:";
    assertion += op2.as_str();
    assertion += "${BAZ}}}\").unwrap());";
    println!("    {assertion}");
    println!("}}");
}

fn foo_bar_baz(idx: usize) -> (Option<String>, Option<String>, Option<String>) {
    let foo = if idx & 1 != 0 {
        Some("foo".to_string())
    } else {
        None
    };
    let bar = if idx & 2 != 0 {
        Some("bar".to_string())
    } else {
        None
    };
    let baz = if idx & 4 != 0 {
        Some("baz".to_string())
    } else {
        None
    };
    (foo, bar, baz)
}

fn main() {
    println!("// AUTO-GENERATED FILE:  DO NOT EDIT MANUALLY (or with sed)");
    println!("// regenerate with:  cargo run --example tests > tests/shvar.rs && cargo fmt");
    println!("use std::collections::HashMap;");
    println!("use shvar::expand;");

    for idx in 0..2 {
        let (foo, _, _) = foo_bar_baz(idx);
        generate_test_case1(foo);
    }

    for idx in 0..4 {
        for op1 in OPS {
            let (foo, bar, _) = foo_bar_baz(idx);
            generate_test_case3(foo, *op1, bar);
        }
    }

    for idx in 0..8 {
        for op1 in OPS {
            for op2 in OPS {
                let (foo, bar, baz) = foo_bar_baz(idx);
                generate_test_case5(foo, *op1, bar, *op2, baz);
            }
        }
    }
}
