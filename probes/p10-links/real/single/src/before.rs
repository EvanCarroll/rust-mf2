//! Declared before the include: the prelude, and a path.
use crate::prelude::*;

pub fn a() -> mf2::Tr {
    tr!("plain")
}

pub fn b() -> mf2::TrArgs {
    let _ = msg_id!("canary.text");
    crate::tr!("greeting", name = "Ada")
}
