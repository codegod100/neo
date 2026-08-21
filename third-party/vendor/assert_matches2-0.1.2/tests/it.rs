use assert_matches2::assert_matches;

#[test]
fn no_error() {
    #[derive(Debug)]
    struct Foo {
        bar: Bar,
    }

    #[derive(Debug)]
    enum Bar {
        V1(u32),
        V2 { field: String },
    }

    let var = Foo { bar: Bar::V1(10) };
    assert_matches!(var, Foo { bar: ref r @ Bar::V1(int) });
    assert_matches!(r, Bar::V1(_));
    assert_eq!(int, 10);

    let var = Foo { bar: Bar::V2 { field: "test".to_owned() } };
    assert_matches!(var, Foo { bar: Bar::V2 { field: rename } });
    assert_eq!(rename, "test");
}

#[test]
#[should_panic(expected = "\
match assertion failed
 pattern: `None`
   value: `Some(8)`\
")]
fn default_error() {
    assert_matches!(Some(8u8), None);
}

#[test]
#[should_panic(expected = "\
match assertion failed: custom error message
 pattern: `Some(8u8)`
   value: `None`\
")]
fn custom_error() {
    let msg = "message";
    assert_matches!(None, Some(8u8), "custom error {msg}");
}
