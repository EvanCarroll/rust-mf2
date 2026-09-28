//! Declared after the include.

#[cfg(feature = "after-glob")]
pub fn after() -> mf2::Tr {
    use v5_prelude::prelude::*;
    tr!("hello")
}

#[cfg(feature = "after-glob-explicit")]
pub fn after_explicit() -> mf2::Tr {
    #[allow(unused_imports)]
    use v5_prelude::prelude::*;
    use crate::tr;
    tr!("hello")
}
