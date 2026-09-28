//! Another crate calls the translation crate's exported `tr!` by path.

pub fn by_path() -> mf2::Tr {
    v1_today::tr!("hello")
}

pub fn by_use() -> mf2::TrArgs {
    use v1_today::tr;
    tr!("greet", name = "Ada")
}
