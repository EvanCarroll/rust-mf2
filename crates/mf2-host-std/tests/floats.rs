//! `StdHost::f64_to_text` against `ryu`, the formatter 2.x shipped.
//!
//! The host now writes `core`'s own shortest round-trip text. `core` and
//! `ryu` switch to an exponent at different magnitudes, which does not
//! matter: what the runtime keeps is the `Number` the text parses to. This
//! test asserts the two agree on that, over the edge values and a large
//! random sample, and that neither host overflows the 32-byte buffer.

use mf2_host_std::HOST;
use mf2_runtime::{Host, Number, Sink};

/// A host that formats floats as 2.x did, the oracle for this test.
struct RyuHost;

impl Host for RyuHost {
    fn nfc<'a>(&self, s: &'a str, buf: &'a mut String) -> &'a str {
        buf.clear();
        buf.push_str(s);
        buf
    }

    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        if !x.is_finite() {
            return None;
        }
        let mut b = ryu::Buffer::new();
        let text = b.format_finite(x).as_bytes();
        let out = buf.get_mut(..text.len())?;
        out.copy_from_slice(text);
        core::str::from_utf8(out).ok()
    }
}

/// The exact value in plain digits: `Number` is opaque and not `PartialEq`,
/// and the plain form shows every digit the parse kept.
fn plain(n: &Number) -> String {
    let mut s = String::new();
    n.write_plain(&mut s as &mut dyn Sink);
    s
}

/// The two plain forms are the same length and differ only in their last
/// digit, by one: the exact value sits halfway between them, so each is as
/// short and as close as the other and the choice between them is free.
fn differ_only_by_a_tie(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a[..a.len() - 1] == b[..b.len() - 1]
        && match (a.as_bytes().last(), b.as_bytes().last()) {
            (Some(p), Some(q)) => p.abs_diff(*q) == 1,
            _ => false,
        }
}

/// Both hosts produce text (so neither overflowed the buffer), `core`'s text
/// round-trips `x`, and the numbers the two parse to are the same — bar an exact tie in the last digit, where both answers are
/// right and the formatters pick differently (about one value in 4,000).
fn agree(x: f64) {
    let mut buf = [0u8; 32];
    let core_text = HOST.f64_to_text(x, &mut buf).map(str::to_owned);
    let mut buf = [0u8; 32];
    let ryu_text = RyuHost.f64_to_text(x, &mut buf).map(str::to_owned);
    assert!(
        core_text.is_some() && ryu_text.is_some(),
        "{x:?}: core {core_text:?}, ryu {ryu_text:?}"
    );
    let from_core = Number::from_f64(x, &HOST).map(|n| plain(&n));
    let from_ryu = Number::from_f64(x, &RyuHost).map(|n| plain(&n));
    let (core_text, ryu_text) = (core_text.unwrap(), ryu_text.unwrap());
    assert_eq!(
        core_text.parse::<f64>().map(f64::to_bits),
        Ok(x.to_bits()),
        "{x:?}"
    );
    let (core_num, ryu_num) = (from_core.unwrap(), from_ryu.unwrap());
    assert!(
        core_num == ryu_num || differ_only_by_a_tie(&core_num, &ryu_num),
        "{x:?}: core {core_text} -> {core_num}, ryu {ryu_text} -> {ryu_num}"
    );
}

#[test]
fn edge_values() {
    let mut values = vec![
        0.0,
        -0.0,
        f64::MIN_POSITIVE,
        -f64::MIN_POSITIVE,
        f64::from_bits(1), // the smallest subnormal
        -f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff), // the largest subnormal
        f64::MAX,
        f64::MIN,
        1.0,
        -1.0,
        4.2,
        -0.5,
        9_007_199_254_740_991.0, // 2^53 - 1
        9_007_199_254_740_992.0, // 2^53
        9_007_199_254_740_994.0, // 2^53 + 2
        -9_007_199_254_740_993.0,
    ];
    // Either side of where each formatter switches to an exponent: `core`
    // at 1e16 and 1e-4, `ryu` by digit count.
    for e in [
        -324i32, -308, -17, -8, -5, -4, -3, 0, 1, 15, 16, 17, 21, 22, 300, 308,
    ] {
        for m in [1.0, 1.234_567_890_123_456_7, 9.999_999_999_999_998] {
            let x = m * 10f64.powi(e);
            if x.is_finite() && x != 0.0 {
                values.push(x);
                values.push(-x);
            }
        }
    }
    for x in values {
        agree(x);
    }
}

#[test]
fn random_sample() {
    // xorshift64*, so the sample is the same on every run and the crate
    // needs no random-number dependency.
    let mut s: u64 = 0x2545_f491_4f6c_dd1d;
    let mut checked = 0u32;
    while checked < 200_000 {
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        let x = f64::from_bits(s.wrapping_mul(0x2545_f491_4f6c_dd1d));
        if x.is_finite() {
            agree(x);
            checked += 1;
        }
    }
}
