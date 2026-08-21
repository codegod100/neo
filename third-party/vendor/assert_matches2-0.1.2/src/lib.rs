#![doc = include_str!("../README.md")]
#![no_std]

/// Assert that the given expression matches the given pattern, and introduce
/// any bindings from the pattern into the surrounding scope.
///
/// If the assertion fails, the expression before the comma is debug-printed as
/// part of the panic message.
#[macro_export]
macro_rules! assert_matches {
    ($e:expr, $pat:pat) => {
        let value = $e;
        let $pat = value else {
            ::core::panic!(
                "match assertion failed\n pattern: `{}`\n   value: `{:?}`",
                ::core::stringify!($pat), value,
            );
        };
    };
    ($e:expr, $pat:pat, $($arg:tt)*) => {
        let value = $e;
        let $pat = value else {
            ::core::panic!(
                "match assertion failed: {}\n pattern: `{}`\n   value: `{:?}`",
                ::core::format_args!($($arg)*), ::core::stringify!($pat), value,
            );
        };
    };
}

/// Alternative form of [`assert_matches!`] where the pattern comes first.
///
/// If the assertion fails, the expression after the `=` is debug-printed as
/// part of the panic message.
///
/// # Example
///
/// ```
/// use assert_matches2::assert_let;
/// use serde_json::{json, Value as JsonValue};
///
/// let val = json!({ "field": [1, 2, 3] });
/// assert_let!(JsonValue::Object(obj) = val);
/// assert_eq!(obj.len(), 1);
/// assert_let!(Some((field_name, JsonValue::Array(arr))) = obj.into_iter().next());
/// assert_eq!(field_name, "field");
/// assert_eq!(arr, [1, 2, 3]);
/// ```
#[macro_export]
macro_rules! assert_let {
    ($pat:pat = $e:expr) => {
        $crate::assert_matches!($e, $pat);
    };
    ($pat:pat = $e:expr, $($arg:tt)*) => {
        $crate::assert_matches!($e, $pat, $($arg)*);
    }
}
