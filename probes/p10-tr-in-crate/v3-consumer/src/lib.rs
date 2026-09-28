//! Another crate names the translation crate's `tr!`.

#[cfg(feature = "by-path")]
pub fn by_path() -> mf2::Tr {
    v3_local::tr!("hello")
}

#[cfg(feature = "by-use")]
pub fn by_use() -> mf2::TrArgs {
    use v3_local::tr;
    tr!("greet", name = "Ada")
}

#[cfg(feature = "by-prelude")]
pub fn by_prelude() -> mf2::Tr {
    use v3_local::prelude::*;
    tr!("hello")
}

#[cfg(feature = "by-exports")]
pub fn by_exports() -> mf2::Tr {
    use v3_local::exports::tr;
    tr!("hello")
}
