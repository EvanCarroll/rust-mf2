//! The content hash in a catalog's file name (`<locale>.<hash>.mf2b`).
//!
//! Build side and native side only (`content-hash`): `mf2-build` names each
//! catalog with it, and `mf2-native` checks a catalog file it reads against
//! the name it was loaded under. The web client never computes it: its
//! catalogs come from its own server or build.

use alloc::string::String;

use sha2::{Digest, Sha256};

/// The number of hex digits of [`content_hash`]: the first 8 bytes of the
/// SHA-256.
pub const CONTENT_HASH_LEN: usize = 16;

/// The first 16 lowercase hex digits of SHA-256 over a catalog's bytes.
///
/// Long enough that two builds of one application never collide, short
/// enough to read in a URL. A catalog is served immutable under this name, so
/// the same bytes must always give the same one — which they do, since the
/// writer is deterministic. Two builds that differ only in a message's text
/// share a manifest hash but not this one.
#[must_use]
pub fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(CONTENT_HASH_LEN);
    for byte in digest.iter().take(CONTENT_HASH_LEN / 2) {
        out.push(hex(byte >> 4));
        out.push(hex(byte & 0xF));
    }
    out
}

fn hex(nibble: u8) -> char {
    char::from_digit(u32::from(nibble), 16).unwrap_or('0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_the_sha256_prefix() {
        // sha256("") = e3b0c44298fc1c14…
        assert_eq!(content_hash(b""), "e3b0c44298fc1c14");
        assert_eq!(content_hash(b"abc"), "ba7816bf8f01cfea");
    }
}
