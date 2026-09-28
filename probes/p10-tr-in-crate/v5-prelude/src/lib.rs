//! The prelude's `tr!` refuses to compile, and says it was the one chosen.

#[doc(hidden)]
#[macro_export]
macro_rules! __prelude_tr {
    ($($t:tt)*) => {
        ::core::compile_error!("the PRELUDE's tr! was chosen")
    };
}

pub mod prelude {
    pub use crate::__prelude_tr as tr;
    pub use mf2::Tr;
}
