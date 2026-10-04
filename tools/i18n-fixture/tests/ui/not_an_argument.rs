//! A value that is neither a number, a string, a date, a path nor a type with
//! `Display`: the error is `IntoArg`'s, at the argument, and names what an
//! argument may be.
struct Opaque;

fn main() {
    let _ = mf2_i18n_fixture::tr!("greeting", name = Opaque);
    // An `Option` is not one either: the call site says what to show.
    let _ = mf2_i18n_fixture::tr!("received", count = Some(3));
}
