//! The key a call site's markup handler is found by.
//!
//! A rich call site carries `(key, handler)` pairs, not names: the macro
//! hashes each markup name at compile time, the renderer hashes the name the
//! catalog gives it ([`crate::view`]'s markup parts) and compares. That is
//! how a markup name stays out of the wasm, exactly as an argument name does
//! (B6). It lives here because both sides — `mf2-macros` and the `mf2`
//! facade — already depend on this crate, and neither depends on the other.

/// FNV-1a 64 over the name's UTF-8 bytes — the manifest hash's function,
/// over a single name.
///
/// Names are NFC on both sides: the manifest's are normalized by the build,
/// and a part's name comes from a catalog the same build wrote.
#[must_use]
pub fn markup_key(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in name.as_bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::markup_key;

    #[test]
    fn a_name_keys_to_itself_and_not_to_another() {
        assert_eq!(markup_key("kbd"), markup_key("kbd"));
        assert_ne!(markup_key("kbd"), markup_key("link"));
        // The empty name is not the offset basis' neighbour by accident.
        assert_eq!(markup_key(""), 0xcbf2_9ce4_8422_2325);
    }
}
