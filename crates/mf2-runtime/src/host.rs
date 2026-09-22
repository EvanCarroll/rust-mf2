//! What the runtime asks of its platform (`plans/03-runtime.md` §2.5): NFC
//! normalization (no tables in the wasm, §7) and the shortest text of a
//! float (no float-printing code in the wasm). `mf2-host-std` implements it
//! natively and for `wasm32-wasip1`, `mf2-host-web` in the browser.

use alloc::string::String;

/// The platform services the runtime needs.
pub trait Host: Sync {
    /// The NFC form of `s`, which failed the quick check (it has a code point
    /// at or above U+0300). `buf` is scratch the result may live in.
    fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str;

    /// The shortest decimal text that round-trips the finite `x`, written
    /// into `buf`: any form `number-literal` accepts, with an optional `+` in
    /// the exponent (`ryu`'s `4.2`, `1e21`, `1.5e-7` and JavaScript's
    /// `String(x)` both qualify). `None` if the host cannot produce it.
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str>;
}
