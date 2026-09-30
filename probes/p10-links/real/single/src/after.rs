//! Declared after the include: `use crate::tr;`.
use crate::tr;

pub fn c() -> mf2::Tr {
    let _ = crate::msg_id!("plain");
    tr!("canary.text")
}
