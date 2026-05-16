use indicio::puzzle_piece;

puzzle_piece!(Type1 {});

#[test]
fn type1() {
    let type1 = Type1 {};
    assert_eq!(Some(type1), Type1::extract(&indicio::value!({})));
}

puzzle_piece!(Type2 {
    foo: String,
    bar: String,
});

#[test]
fn type2() {
    let type2 = Type2 {
        foo: "foo".to_string(),
        bar: "bar".to_string(),
    };
    assert_eq!(
        Some(type2),
        Type2::extract(&indicio::value!({
            foo: "foo",
            bar: "bar",
        }))
    );
}

puzzle_piece!(
    Type3 {
        field1: {
            foo: String,
            bar: String,
        },
        baz: String,
    }
);

#[test]
fn type3() {
    let type3 = Type3 {
        foo: "foo".to_string(),
        bar: "bar".to_string(),
        baz: "baz".to_string(),
    };
    assert_eq!(
        Some(type3),
        Type3::extract(&indicio::value!({
            field1: {
                foo: "foo",
                bar: "bar",
            },
            baz: "baz",
        }))
    );
}

puzzle_piece!(
    Type4 {
        baz: String,
        field2: {
            foo: String,
            bar: String,
        },
    }
);

#[test]
fn type4() {
    let type4 = Type4 {
        foo: "foo".to_string(),
        bar: "bar".to_string(),
        baz: "baz".to_string(),
    };
    assert_eq!(
        Some(type4),
        Type4::extract(&indicio::value!({
            baz: "baz",
            field2: {
                foo: "foo",
                bar: "bar",
            },
        }))
    );
}

puzzle_piece!(
    Type5 {
        nested: {
            foo: String
        }
    }
);

#[test]
fn nested_terminal_without_trailing_comma() {
    let type5 = Type5 {
        foo: "foo".to_string(),
    };
    assert_eq!(
        Some(type5),
        Type5::extract(&indicio::value!({
            nested: {
                foo: "foo",
            },
        }))
    );
}

#[test]
fn try_extract_reports_missing_path() {
    assert_eq!(
        indicio::ExtractionError::missing(&["field1", "bar"]),
        Type3::try_extract(&indicio::value!({
            field1: {
                foo: "foo",
            },
            baz: "baz",
        }))
        .unwrap_err()
    );
}

#[test]
fn try_extract_reports_type_mismatch() {
    assert_eq!(
        indicio::ExtractionError::type_mismatch(&["foo"], "String", indicio::ValueKind::I64),
        Type2::try_extract(&indicio::value!({
            foo: 42,
            bar: "bar",
        }))
        .unwrap_err()
    );
}

mod qualified {
    indicio::puzzle_piece!(QualifiedType {
        outer: {
            answer: i64,
        },
    });

    #[test]
    fn qualified_macro_invocation() {
        assert_eq!(
            Some(QualifiedType { answer: 42 }),
            QualifiedType::extract(&indicio::value!({
                outer: {
                    answer: 42i64,
                },
            }))
        );
    }
}
