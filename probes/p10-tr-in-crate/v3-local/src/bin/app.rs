//! 3c in a binary crate (a single-crate application): a module declared
//! before the include, one after, and the crate root.

mod ui {
    use crate::prelude::*;

    pub fn title() -> mf2::Tr {
        tr!("hello")
    }
}

mf2::include_generated!("mf2_generated_v3.rs");

mod status {
    use crate::tr;

    pub fn line() -> mf2::TrArgs {
        tr!("greet", name = "Ada")
    }
}

fn main() {
    let _ = (ui::title(), status::line(), tr!("hello"));
}
