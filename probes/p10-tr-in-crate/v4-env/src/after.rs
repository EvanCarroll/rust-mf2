//! Declared after the include.

use v4_env_macro::{tr_env, tr_outdir};

pub fn env_greet() -> mf2::TrArgs {
    tr_env!("greet", name = "Ada")
}

pub fn outdir_hello() -> mf2::Tr {
    tr_outdir!("hello")
}
