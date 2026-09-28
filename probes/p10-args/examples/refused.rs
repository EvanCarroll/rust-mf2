//! Each feature turns on one call that must not compile.

#[allow(unused_imports)]
use p10_args::arg;

/// Neither `IntoArg` nor `Display`.
#[allow(dead_code)]
struct Opaque;

#[cfg(feature = "neither")]
fn neither() {
    let _ = arg!(Opaque);
}

#[cfg(feature = "neither-generic")]
fn neither_generic<T>(t: T) {
    let _ = arg!(t);
}

#[cfg(feature = "option")]
fn option() {
    let _ = arg!(Some(3_i64));
}

fn main() {
    #[cfg(feature = "neither")]
    neither();
    #[cfg(feature = "neither-generic")]
    neither_generic(1u8);
    #[cfg(feature = "option")]
    option();
}
