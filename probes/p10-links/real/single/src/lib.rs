//! A3's call sites in the crate that includes the module (plans/19 §12).

mod before;
mf2::include_generated!();
mod after;
#[cfg(feature = "forget")]
mod forgets;

/// The crate root, after the include: no import.
pub fn root() -> mf2::Tr {
    tr!("plain")
}

/// What each place formats.
#[cfg(feature = "native")]
pub fn all() -> [String; 4] {
    [
        before::a().to_string(),
        before::b().to_string(),
        after::c().to_string(),
        root().to_string(),
    ]
}
