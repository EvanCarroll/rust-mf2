//! Numbers through the browser (`plans/03-runtime.md` §2.7, "the `intl`
//! option"; §5.3): a [`NumberFormatter`] whose `format` is
//! `Intl.NumberFormat` (`format`, and `formatToParts` for sub-parts) and
//! whose `plural` is `Intl.PluralRules`, given by `Host::numbers` only when
//! the engine has `Intl.NumberFormat` v3, which every MF2 digit option
//! needs (owner decision 4) — detected once and remembered.
//!
//! One inline-JS module caches one `Intl.NumberFormat` and one
//! `Intl.PluralRules` per locale and option set (FIFO, 256: a message whose
//! options come from variables must not keep one object per value alive).
//! The options travel as a JSON object built here into a fixed buffer (no
//! allocation, no `core::fmt`) and are parsed once per cache entry; the
//! value travels as its exact decimal text, which `Intl.NumberFormat` v3
//! formats exactly (`Intl.PluralRules.select` converts it to a JS number).
//!
//! The mapping: MF2's option names and values are ECMA-402's. A neutral
//! request is locale `en` with `numberingSystem: "latn"` and `useGrouping:
//! false`; `currencyDisplay=never` formats with `code` and drops the
//! `currency` part and the spacing next to it; digit options equal to
//! `Intl`'s defaults for the style are left out.

use core::sync::atomic::{AtomicU8, Ordering};

use mf2_runtime::{
    Category, CurrencyDisplay, DateTimeRequest, DigitOptions, Grouping, Host, NumberFormatter,
    NumberOut, NumberRequest, NumberStyle, RoundingMode, RoundingPriority, SignDisplay, Sink,
    UnitDisplay,
};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::HOST;

/// A host with numbers through `Intl` (feature `intl`): the number methods
/// here, every other method from the host it wraps — [`crate::HOST`], or
/// a date host (`IntlNumbers(&INTL_HOST)` for a corpus with dates under
/// `datetime-intl`). Its own static, like the date hosts, so a client that
/// formats no numbers names a host without this glue (B1′). It forwards
/// each `Host` method; a method added to the trait later must be forwarded
/// here too.
#[derive(Clone, Copy)]
pub struct IntlNumbers(pub &'static dyn Host);

/// [`IntlNumbers`] over [`crate::HOST`], for
/// [`mf2_runtime::FormatContext::new`].
pub static NUMBERS_HOST: IntlNumbers = IntlNumbers(&HOST);

impl Host for IntlNumbers {
    fn f64_to_text<'b>(&self, x: f64, buf: &'b mut [u8; 32]) -> Option<&'b str> {
        self.0.f64_to_text(x, buf)
    }

    fn zone_offset(&self, zone: &str, epoch_ms: i64) -> Option<i32> {
        self.0.zone_offset(zone, epoch_ms)
    }

    fn format_date_time(
        &self,
        locale: &str,
        request: &DateTimeRequest<'_>,
        out: &mut dyn Sink,
    ) -> bool {
        self.0.format_date_time(locale, request, out)
    }

    fn numbers(&self) -> Option<&dyn NumberFormatter> {
        available().then_some(&INTL)
    }
}

/// `Intl.NumberFormat` and `Intl.PluralRules`, for an engine with
/// `Intl.NumberFormat` v3 ([`IntlNumbers`] hands it out only then).
struct Intl;

static INTL: Intl = Intl;

impl NumberFormatter for Intl {
    fn format(&self, locale: &str, request: &NumberRequest<'_>, out: NumberOut<'_>) -> bool {
        format(locale, request, out)
    }

    fn plural(&self, locale: &str, request: &NumberRequest<'_>) -> Option<Category> {
        plural(locale, request)
    }
}

// The JavaScript, minified by hand (it ships as written; A0 measured its
// size, bench/intl-probe). `g(cache, key, plural)`: the cached
// `Intl.PluralRules` or `Intl.NumberFormat` for a key `locale U+0001 JSON
// options` (one string across the boundary, parsed only on a miss; FIFO, 256
// per cache; `null` when the engine rejects the set), with
// `currencyDisplay: "never"` mapped to `code` and `h` set. `mf2_nf`: the text,
// or the parts as `type U+001F value` records separated by U+001E, dropping
// for `h` the currency and a blank literal next to it; `undefined` when
// rejected. `mf2_pr`: the category's code (0 zero … 5 other; 6 rejected).
// `mf2_nf_v3`: `Intl.NumberFormat` v3 by behaviour, not `resolvedOptions()`
// (Chromium 143 resolves `roundingPriority` to "auto" yet honours it) — each
// test fails on an engine that ignores the option.
#[wasm_bindgen(inline_js = r#"
const N=new Map,P=new Map,K={zero:0,one:1,two:2,few:3,many:4,other:5};
function g(c,y,k){let f=c.get(y);if(f===void 0){const i=y.indexOf('\x01'),l=y.slice(0,i),o=JSON.parse(y.slice(i+1)),h=o.currencyDisplay=='never';h&&(o.currencyDisplay='code');try{f={i:k?new Intl.PluralRules(l,o):new Intl.NumberFormat(l,o),h}}catch{f=null}c.size>255&&c.delete(c.keys().next().value);c.set(y,f)}return f}
export function mf2_nf(y,v,p){const f=g(N,y,0);if(f===null)return;if(!p&&!f.h)return f.i.format(v);const q=f.i.formatToParts(v);let r='';for(let i=0;i<q.length;i++){const t=q[i];if(f.h&&(t.type=='currency'||t.type=='literal'&&!t.value.trim()&&(q[i-1]?.type=='currency'||q[i+1]?.type=='currency')))continue;r+=p?(r?'\x1e':'')+t.type+'\x1f'+t.value:t.value}return r}
export function mf2_pr(y,v){const f=g(P,y,1);return f===null?6:K[f.i.select(+v)]}
export function mf2_nf_v3(){try{const f=(o,v)=>new Intl.NumberFormat('en',o).format(v);return f({maximumFractionDigits:1,maximumSignificantDigits:1,roundingPriority:'morePrecision'},1.25)=='1.3'&&f({maximumFractionDigits:0,roundingMode:'halfEven'},2.5)=='2'&&f({minimumFractionDigits:2,maximumFractionDigits:2,roundingIncrement:5},'1.23')=='1.25'&&f({minimumFractionDigits:2,trailingZeroDisplay:'stripIfInteger'},1)=='1'&&f({useGrouping:'min2'},1e3)=='1000'&&f({signDisplay:'negative'},-0)=='0'&&f({useGrouping:!1,maximumFractionDigits:20},'9007199254740993.5')=='9007199254740993.5'&&new Intl.PluralRules('en',{maximumFractionDigits:0,roundingMode:'floor'}).select(1.9)=='one'}catch{return!1}}
"#)]
extern "C" {
    fn mf2_nf(key: &str, value: &str, parts: bool) -> Option<alloc::string::String>;
    fn mf2_pr(key: &str, value: &str) -> u32;
    fn mf2_nf_v3() -> bool;
}

/// 0: not yet detected, 1: `Intl.NumberFormat` v3, 2: not.
static V3: AtomicU8 = AtomicU8::new(0);

/// Whether the engine has `Intl.NumberFormat` v3 (asked once).
fn available() -> bool {
    match V3.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            let yes = mf2_nf_v3();
            V3.store(if yes { 1 } else { 2 }, Ordering::Relaxed);
            yes
        }
    }
}

/// A formatter's key — the locale, U+0001, and its options as a JSON
/// object — written into a fixed buffer; too long → `None`. The longest
/// option set, a 64-byte unit identifier with every option at its longest
/// value, is 451 bytes; the rest holds the locale tag.
struct Json {
    buf: [u8; 640],
    len: usize,
    ok: bool,
    fields: u8,
}

impl Json {
    fn new(locale: &str) -> Json {
        let mut j = Json {
            buf: [0; 640],
            len: 0,
            ok: true,
            fields: 0,
        };
        j.raw(locale);
        j.raw("\u{1}");
        j
    }

    fn raw(&mut self, s: &str) {
        for &b in s.as_bytes() {
            let Some(slot) = self.buf.get_mut(self.len) else {
                self.ok = false;
                return;
            };
            *slot = b;
            self.len += 1;
        }
    }

    fn key(&mut self, k: &str) {
        self.raw(if self.fields == 0 { "{\"" } else { ",\"" });
        self.fields = self.fields.saturating_add(1);
        self.raw(k);
        self.raw("\":");
    }

    /// `"k":"v"` — `v` needs no escaping: option words, currency codes and
    /// unit identifiers are ASCII letters, digits and `-`.
    fn text(&mut self, k: &str, v: &str) {
        self.key(k);
        self.raw("\"");
        self.raw(v);
        self.raw("\"");
    }

    fn number(&mut self, k: &str, v: u16) {
        self.key(k);
        let mut d = [0u8; 5];
        let mut at = d.len();
        let mut v = v;
        loop {
            at -= 1;
            if let Some(slot) = d.get_mut(at) {
                // `v % 10 < 10`.
                #[allow(clippy::cast_possible_truncation)]
                {
                    *slot = b'0' + (v % 10) as u8;
                }
            }
            v /= 10;
            if v == 0 || at == 0 {
                break;
            }
        }
        self.raw(core::str::from_utf8(d.get(at..).unwrap_or(&[])).unwrap_or("0"));
    }

    fn finish(&mut self) -> Option<&str> {
        self.raw(if self.fields == 0 { "{}" } else { "}" });
        if !self.ok {
            return None;
        }
        core::str::from_utf8(self.buf.get(..self.len)?).ok()
    }
}

fn mode_name(m: RoundingMode) -> &'static str {
    match m {
        RoundingMode::Ceil => "ceil",
        RoundingMode::Floor => "floor",
        RoundingMode::Expand => "expand",
        RoundingMode::Trunc => "trunc",
        RoundingMode::HalfCeil => "halfCeil",
        RoundingMode::HalfFloor => "halfFloor",
        RoundingMode::HalfTrunc => "halfTrunc",
        RoundingMode::HalfEven => "halfEven",
        // `halfExpand`, the default, and (not exhaustive) a mode this
        // version does not know; as below, an unknown value is the default.
        _ => "halfExpand",
    }
}

/// The digit options, those equal to `Intl`'s defaults for a style whose
/// default fraction digits are `defaults` (`None`: a currency's own, so
/// always written) left out.
fn digits(d: &DigitOptions, defaults: Option<(u8, u8)>, j: &mut Json) {
    if d.minimum_integer != 1 {
        j.number("minimumIntegerDigits", d.minimum_integer.into());
    }
    if let Some((mn, mx)) = d.fraction
        && defaults != Some((mn, mx))
    {
        j.number("minimumFractionDigits", mn.into());
        j.number("maximumFractionDigits", mx.into());
    }
    if let Some((mn, mx)) = d.significant {
        j.number("minimumSignificantDigits", mn.into());
        j.number("maximumSignificantDigits", mx.into());
    }
    match d.priority {
        RoundingPriority::MorePrecision => j.text("roundingPriority", "morePrecision"),
        RoundingPriority::LessPrecision => j.text("roundingPriority", "lessPrecision"),
        _ => {}
    }
    if d.increment != 1 {
        j.number("roundingIncrement", d.increment);
    }
    if d.mode != RoundingMode::HalfExpand {
        j.text("roundingMode", mode_name(d.mode));
    }
    if d.strip_if_integer {
        j.text("trailingZeroDisplay", "stripIfInteger");
    }
}

/// The `Intl.NumberFormat` options of `r`.
fn format_options(r: &NumberRequest<'_>, j: &mut Json) {
    let defaults = match r.style {
        NumberStyle::Percent => {
            j.text("style", "percent");
            Some((0, 0))
        }
        NumberStyle::Currency {
            code,
            display,
            accounting,
            ..
        } => {
            j.text("style", "currency");
            j.text("currency", code);
            let d = match display {
                CurrencyDisplay::NarrowSymbol => Some("narrowSymbol"),
                CurrencyDisplay::Name => Some("name"),
                CurrencyDisplay::Code => Some("code"),
                CurrencyDisplay::Never => Some("never"),
                // `symbol`, the default, or a value this version does not know.
                _ => None,
            };
            if let Some(d) = d {
                j.text("currencyDisplay", d);
            }
            if accounting {
                j.text("currencySign", "accounting");
            }
            None
        }
        NumberStyle::Unit { unit, display } => {
            j.text("style", "unit");
            j.text("unit", unit);
            match display {
                UnitDisplay::Narrow => j.text("unitDisplay", "narrow"),
                UnitDisplay::Long => j.text("unitDisplay", "long"),
                _ => {}
            }
            Some((0, 3))
        }
        _ => Some((0, 3)),
    };
    if r.neutral {
        j.text("numberingSystem", "latn");
        j.key("useGrouping");
        j.raw("false");
    } else {
        match r.grouping {
            Grouping::Always => j.text("useGrouping", "always"),
            Grouping::Min2 => j.text("useGrouping", "min2"),
            Grouping::Never => {
                j.key("useGrouping");
                j.raw("false");
            }
            _ => {}
        }
    }
    let sign = match r.sign {
        SignDisplay::Always => Some("always"),
        SignDisplay::ExceptZero => Some("exceptZero"),
        SignDisplay::Negative => Some("negative"),
        SignDisplay::Never => Some("never"),
        // `auto`, the default, or a value this version does not know.
        _ => None,
    };
    if let Some(s) = sign {
        j.text("signDisplay", s);
    }
    digits(&r.digits, defaults, j);
}

/// Formats `r` for `locale` with `Intl.NumberFormat`; `false` when the
/// engine rejects the option set (a unit it does not sanction).
fn format(locale: &str, r: &NumberRequest<'_>, out: NumberOut<'_>) -> bool {
    let mut j = Json::new(if r.neutral { "en" } else { locale });
    format_options(r, &mut j);
    let Some(key) = j.finish() else {
        return false;
    };
    let parts = matches!(out, NumberOut::Parts(_));
    let Some(text) = mf2_nf(key, r.value, parts) else {
        return false;
    };
    match out {
        NumberOut::Text(out) => out.push_str(&text),
        NumberOut::Parts(out) => {
            // Records `type U+001F value`, separated by U+001E: ASCII bytes,
            // so every split point is a char boundary.
            let mut rest = text.as_str();
            while !rest.is_empty() {
                let end = rest.bytes().position(|b| b == 0x1e).unwrap_or(rest.len());
                let record = rest.get(..end).unwrap_or("");
                if let Some(i) = record.bytes().position(|b| b == 0x1f) {
                    out.sub_part(
                        record.get(..i).unwrap_or(""),
                        record.get(i + 1..).unwrap_or(""),
                    );
                }
                rest = rest.get(end + 1..).unwrap_or("");
            }
        }
    }
    true
}

/// The plural category of `r.value` for `locale` with `Intl.PluralRules`
/// (the request's digit options and type); `None` when the engine rejects
/// them.
fn plural(locale: &str, r: &NumberRequest<'_>) -> Option<Category> {
    let mut j = Json::new(locale);
    if r.ordinal {
        j.text("type", "ordinal");
    }
    digits(&r.digits, Some((0, 3)), &mut j);
    let key = j.finish()?;
    let c = mf2_pr(key, r.value);
    // 0..=5 fit a u8.
    #[allow(clippy::cast_possible_truncation)]
    (c < 6).then(|| Category::from_code(c as u8))
}
