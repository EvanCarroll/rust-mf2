//! **The `Host` methods the `intl` option would add**, kept in this probe-local module: the probe does not change
//! `mf2_runtime::Host`. In the option they would be `Host` methods with
//! default bodies (the Rust path, what `mf2-host-std` keeps on the server)
//! that `mf2-host-web` implements with this JavaScript:
//!
//! | Here | What it does in the browser |
//! |---|---|
//! | [`format`] | `Intl.NumberFormat(locale, options).format(decimalString)` |
//! | [`parts`] | the same, `formatToParts` (a placeholder's sub-parts) |
//! | [`plural`] | `Intl.PluralRules(locale, digitOptions).select(Number(decimalString))` |
//! | [`supported`] | whether `Intl` accepts the option set (`:unit`'s sanctioned units) |
//!
//! The option set crosses as a short key string (two designs below, both
//! measured: B, the default, carries `name=value` pairs; A, feature
//! `key-codes`, carries the catalog's locale, U+0001, 17 one-character
//! fields — code units 0–127 — the currency code, U+0001, the unit) and the
//! JavaScript side caches one
//! `Intl.NumberFormat` and one `Intl.PluralRules` per key (FIFO, 256 keys:
//! a random corpus must not keep one object per option set alive). The
//! value crosses as its exact decimal text: `Intl.NumberFormat` v3 formats a
//! decimal string exactly; `Intl.PluralRules.select` converts it to a JS
//! number (the known precision cost). Every call is one wasm → JS crossing
//! with two strings in (and one out for `format`).
//!
//! The JavaScript is written compact on purpose (it is measured as it
//! ships). Design A's logic, field by field: `E(k)` finds or makes the cache entry
//! for key `k` — the digit options `d` from fields 2–10 (127 = unset;
//! increment by index; mode by index), the locale `l`, the fields `b`, the
//! currency `c` and unit `u`. `F(e)` makes the `NumberFormat` once: neutral
//! (field 1) is locale `en` with `numberingSystem: "latn"` and
//! `useGrouping: false`, otherwise field 12 sets `useGrouping`; field 11
//! `signDisplay`; field 0 the style, with fields 13–15 (`currencyDisplay`,
//! `currencySign`, `unitDisplay`); `currencyDisplay=never` (13 = 4) formats
//! with `code` and drops the currency part and its spacing. `mf2nf(k, v, p)`
//! formats `v`, or with `p` returns its parts as `type U+001F value`
//! records separated by U+001E. `mf2pr(k, v)` makes the `PluralRules` once
//! (field 16: ordinal) and returns the category's code (6: rejected).
//! `mf2ok(k)`: the `NumberFormat` could be made.

use mf2_runtime::{Category, Sink, SubPartSink};
use wasm_bindgen::prelude::wasm_bindgen;

/// Design A: the number of one-character fields after the locale.
#[cfg(feature = "key-codes")]
pub(crate) const KEY_FIELDS: usize = 17;

// Design B (default): `locale U+0001 [numberFormatLocale] U+0001
// name=value,…` — the names and values come from the option tables the
// wasm holds anyway for MF2 (MF2's option names are Intl's), so this
// JavaScript is generic: `E(k)` parses the pairs once per key (digits become
// numbers, `false` a boolean; `currencyDisplay=never` becomes `code` plus
// the drop-the-currency flag `h`); `F(e)` makes the `NumberFormat` for the
// second locale (neutral: `en`) or the first; `mf2pr` gives the whole
// option object to `PluralRules`, which reads only its own options (`type`,
// the digit options) — `NumberFormat` ignores `type`.
#[cfg(not(feature = "key-codes"))]
#[wasm_bindgen(
    inline_js = r"const C=new Map,K={zero:0,one:1,two:2,few:3,many:4,other:5};
function E(k){let e=C.get(k);if(!e){const[l,n,s]=k.split('\x01'),o={};for(const p of s.split(','))if(p){const i=p.indexOf('='),v=p.slice(i+1);o[p.slice(0,i)]=v=='false'?!1:/^\d+$/.test(v)?+v:v}const h=o.currencyDisplay=='never';h&&(o.currencyDisplay='code');e={l,n:n||l,o,h};C.size>255&&C.delete(C.keys().next().value);C.set(k,e)}return e}
function F(e){if(e.f===void 0)try{e.f=new Intl.NumberFormat(e.n,e.o)}catch{e.f=null}return e.f}
export function mf2nf(k,v,p){const e=E(k),n=F(e);if(n===null)return;if(!p&&!e.h)return n.format(v);const q=n.formatToParts(v);let s='';for(let i=0;i<q.length;i++){const t=q[i];if(e.h&&(t.type=='currency'||t.type=='literal'&&!t.value.trim()&&(q[i-1]?.type=='currency'||q[i+1]?.type=='currency')))continue;s+=p?(s?'\x1e':'')+t.type+'\x1f'+t.value:t.value}return s}
export function mf2pr(k,v){const e=E(k);if(e.p===void 0)try{e.p=new Intl.PluralRules(e.l,e.o)}catch{e.p=null}return e.p===null?6:K[e.p.select(+v)]}
export function mf2ok(k){return F(E(k))!==null}"
)]
extern "C" {
    #[wasm_bindgen(js_name = mf2nf)]
    fn js_format(key: &str, value: &str, parts: bool) -> Option<alloc::string::String>;
    #[wasm_bindgen(js_name = mf2pr)]
    fn js_plural(key: &str, value: &str) -> u32;
    #[wasm_bindgen(js_name = mf2ok)]
    fn js_supported(key: &str) -> bool;
}

// Design A (feature `key-codes`, wasm variant `intl-codes`): 17 one-character
// codes after the locale (module documentation); this JavaScript holds
// Intl's option names and values itself.
#[cfg(feature = "key-codes")]
#[wasm_bindgen(
    inline_js = r"const C=new Map,M=['ceil','floor','expand','trunc','halfCeil','halfFloor','halfExpand','halfTrunc','halfEven'],I=[1,2,5,10,20,25,50,100,200,250,500,1e3,2e3,2500,5e3],S=[,'always','exceptZero','negative','never'],G=[,'always',!1,'min2'],D=[,'narrowSymbol','name','code','code'],U=[,'narrow','long'],T=['decimal','percent','currency','unit'],K={zero:0,one:1,two:2,few:3,many:4,other:5};
function E(k){let e=C.get(k);if(!e){const i=k.indexOf('\x01'),b=[],d={};for(let j=0;j<17;j++)b[j]=k.charCodeAt(i+1+j);const r=k.slice(i+18),j=r.indexOf('\x01');d.minimumIntegerDigits=b[2];b[3]<127&&(d.minimumFractionDigits=b[3]);b[4]<127&&(d.maximumFractionDigits=b[4]);b[5]<127&&(d.minimumSignificantDigits=b[5]);b[6]<127&&(d.maximumSignificantDigits=b[6]);b[7]&&(d.roundingPriority=b[7]<2?'morePrecision':'lessPrecision');d.roundingIncrement=I[b[8]];d.roundingMode=M[b[9]];b[10]&&(d.trailingZeroDisplay='stripIfInteger');e={l:k.slice(0,i),b,d,c:r.slice(0,j),u:r.slice(j+1)};C.size>255&&C.delete(C.keys().next().value);C.set(k,e)}return e}
function F(e){if(e.n===void 0){const b=e.b,o={...e.d,style:T[b[0]]};let l=e.l;b[1]?(l='en',o.numberingSystem='latn',o.useGrouping=!1):b[12]&&(o.useGrouping=G[b[12]]);b[11]&&(o.signDisplay=S[b[11]]);b[0]==2&&(o.currency=e.c,b[13]&&(o.currencyDisplay=D[b[13]]),b[14]&&(o.currencySign='accounting'));b[0]==3&&(o.unit=e.u,b[15]&&(o.unitDisplay=U[b[15]]));try{e.n=new Intl.NumberFormat(l,o)}catch{e.n=null}}return e.n}
export function mf2nf(k,v,p){const e=E(k),n=F(e);if(n===null)return;if(!p&&e.b[13]!=4)return n.format(v);const q=n.formatToParts(v);let s='';for(let i=0;i<q.length;i++){const t=q[i];if(e.b[13]==4&&(t.type=='currency'||t.type=='literal'&&!t.value.trim()&&(q[i-1]?.type=='currency'||q[i+1]?.type=='currency')))continue;s+=p?(s?'\x1e':'')+t.type+'\x1f'+t.value:t.value}return s}
export function mf2pr(k,v){const e=E(k);if(e.p===void 0)try{e.p=new Intl.PluralRules(e.l,{...e.d,type:e.b[16]?'ordinal':'cardinal'})}catch{e.p=null}return e.p===null?6:K[e.p.select(+v)]}
export function mf2ok(k){return F(E(k))!==null}"
)]
extern "C" {
    #[wasm_bindgen(js_name = mf2nf)]
    fn js_format(key: &str, value: &str, parts: bool) -> Option<alloc::string::String>;
    #[wasm_bindgen(js_name = mf2pr)]
    fn js_plural(key: &str, value: &str) -> u32;
    #[wasm_bindgen(js_name = mf2ok)]
    fn js_supported(key: &str) -> bool;
}

/// Writes `value` formatted under `key`; `false` when `Intl` rejects the
/// option set (nothing written).
pub(crate) fn format(key: &str, value: &str, out: &mut dyn Sink) -> bool {
    match js_format(key, value, false) {
        Some(s) => {
            out.push_str(&s);
            true
        }
        None => false,
    }
}

/// Writes the `formatToParts` parts of `value` under `key`.
pub(crate) fn parts(key: &str, value: &str, out: &mut dyn SubPartSink) {
    let Some(s) = js_parts_text(key, value) else {
        return;
    };
    // Records `type U+001F value`, separated by U+001E (ASCII bytes, so
    // every split point is a char boundary).
    let mut rest = s.as_str();
    while !rest.is_empty() {
        let end = byte_at(rest, 0x1e).unwrap_or(rest.len());
        let record = rest.get(..end).unwrap_or("");
        if let Some(i) = byte_at(record, 0x1f) {
            out.sub_part(
                record.get(..i).unwrap_or(""),
                record.get(i + 1..).unwrap_or(""),
            );
        }
        rest = rest.get(end + 1..).unwrap_or("");
    }
}

fn byte_at(s: &str, b: u8) -> Option<usize> {
    s.bytes().position(|c| c == b)
}

fn js_parts_text(key: &str, value: &str) -> Option<alloc::string::String> {
    js_format(key, value, true)
}

/// The plural category of `value` under `key` (its locale, digit options
/// and plural type); `None` when `Intl` rejects the option set.
pub(crate) fn plural(key: &str, value: &str) -> Option<Category> {
    let c = js_plural(key, value);
    // 0..=5 fit a u8.
    #[allow(clippy::cast_possible_truncation)]
    (c < 6).then(|| Category::from_code(c as u8))
}

/// Whether `Intl` accepts the option set of `key`.
pub(crate) fn supported(key: &str) -> bool {
    js_supported(key)
}
