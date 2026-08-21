# assert_matches2

A replacement for the `assert_matches` crate, providing an `assert_matches!`
macro without support for `if` guards and the extra arm, but bringing any
names introduced in the pattern into scope for after the macro invocation, by
expanding to `let else`.

## Example

```rust
use assert_matches2::assert_matches;

/// Basic usage
let var = serde_json::Value::from("a string");
assert_matches!(var, serde_json::Value::String(s));
assert_eq!(s, "a string");

// More complex usages
#[derive(Debug)]
struct Foo {
    bar: Bar,
}

#[derive(Debug)]
enum Bar {
    V1(u8),
    V2 { field: String },
}

let var = Foo { bar: Bar::V1(10) };
assert_matches!(var, Foo { bar: ref r @ Bar::V1(int) });
assert_matches!(r, Bar::V1(_));
assert_eq!(int, 10);

let var = Foo { bar: Bar::V2 { field: "test".to_owned() } };
assert_matches!(var, Foo { bar: Bar::V2 { field: rename } });
assert_eq!(rename, "test");
```
