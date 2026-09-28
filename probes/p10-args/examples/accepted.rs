//! Every line compiles, and each takes the step its comment names.

use std::fmt;
use std::net::Ipv4Addr;
use std::path::PathBuf;

use p10_args::{ArgValue, IntoArg, arg};

/// A Display-only type of the application (trippy's `KeyBinding`).
struct KeyBinding(char);
impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ctrl+{}", self.0)
    }
}

/// A type with both: `IntoArg` must win.
struct Meters(f64);
impl fmt::Display for Meters {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} m", self.0)
    }
}
impl IntoArg for Meters {
    fn into_arg(self) -> ArgValue {
        ArgValue::Float(self.0)
    }
}

/// Generic code: the step is chosen from the bound in scope.
fn generic_display<T: fmt::Display>(t: T) -> ArgValue {
    arg!(t)
}
fn generic_both<T: fmt::Display + IntoArg>(t: T) -> ArgValue {
    arg!(t)
}

fn main() {
    let n: i64 = 3;
    let dir = PathBuf::from("locales");
    let err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
    let cases = [
        ("i64 → IntoArg", arg!(n), ArgValue::Int(3)),
        ("&i64 → IntoArg (&T, T: Copy)", arg!(&n), ArgValue::Int(3)),
        ("usize → IntoArg", arg!(7usize), ArgValue::Int(7)),
        (
            "String → IntoArg",
            arg!(String::from("a")),
            ArgValue::Str("a".into()),
        ),
        (
            "&str → IntoArg",
            arg!(dir.to_str().unwrap_or("")),
            ArgValue::Str("locales".into()),
        ),
        (
            "&Path → IntoArg",
            arg!(dir.as_path()),
            ArgValue::Str("locales".into()),
        ),
        (
            "io::Error → Display",
            arg!(err),
            ArgValue::Str("no such file".into()),
        ),
        (
            "Ipv4Addr → Display",
            arg!(Ipv4Addr::LOCALHOST),
            ArgValue::Str("127.0.0.1".into()),
        ),
        (
            "KeyBinding → Display",
            arg!(KeyBinding('q')),
            ArgValue::Str("Ctrl+q".into()),
        ),
        (
            "&KeyBinding → Display",
            arg!(&KeyBinding('x')),
            ArgValue::Str("Ctrl+x".into()),
        ),
        (
            "Meters (both) → IntoArg",
            arg!(Meters(2.5)),
            ArgValue::Float(2.5),
        ),
        (
            "generic T: Display → Display",
            generic_display(5_i64),
            ArgValue::Str("5".into()),
        ),
        (
            "generic T: Display + IntoArg → IntoArg",
            generic_both(5_i64),
            ArgValue::Int(5),
        ),
    ];
    let mut ok = true;
    for (what, got, want) in cases {
        let pass = got == want;
        ok &= pass;
        println!("{} {what}: {got:?}", if pass { "PASS" } else { "FAIL" });
    }
    std::process::exit(if ok { 0 } else { 1 });
}
