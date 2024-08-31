use indicio::puzzle_piece;

puzzle_piece!(
    Type1 {
    }
);

#[test]
fn type1() {
    let type1 = Type1 {};
    assert_eq!(Some(type1), Type1::extract(&indicio::value!({})));
}

puzzle_piece!(
    Type2 {
        foo: String,
        bar: String,
    }
);

#[test]
fn type2() {
    let type2 = Type2 {
        foo: "foo".to_string(),
        bar: "bar".to_string(),
    };
    assert_eq!(Some(type2), Type2::extract(&indicio::value!({
        foo: "foo",
        bar: "bar",
    })));
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
    assert_eq!(Some(type3), Type3::extract(&indicio::value!({
        field1: {
            foo: "foo",
            bar: "bar",
        },
        baz: "baz",
    })));
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
    assert_eq!(Some(type4), Type4::extract(&indicio::value!({
        baz: "baz",
        field2: {
            foo: "foo",
            bar: "bar",
        },
    })));
}
