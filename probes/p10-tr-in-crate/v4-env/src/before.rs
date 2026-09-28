//! Declared before the include.

use v4_env_macro::{tr_env, tr_outdir};

pub fn env_hello() -> mf2::Tr {
    tr_env!("hello")
}

pub fn outdir_greet() -> mf2::TrArgs {
    tr_outdir!("greet", name = "Ada")
}
