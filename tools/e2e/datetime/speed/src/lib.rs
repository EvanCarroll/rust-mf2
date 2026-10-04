//! The speed of one date placeholder in a browser (plan/08 §9), browser side:
//! the catalogs of `target/e2e-datetime/speed.json` (one message each, a
//! date argument `$d`), formatted `iters` times in a loop inside wasm — no
//! JS ↔ wasm crossing of the harness per format, `NoErrors`, a reused
//! `String`, the argument cycling through the native side's values — with
//! the formatter this build was made with (one cargo feature per build):
//!
//! | Feature | Formatter | Host |
//! |---|---|---|
//! | `intl` | `Intl.DateTimeFormat` (`mf2-host-web`) | `INTL_HOST` |
//! | `icu` | ICU4X over the catalog's `icu.blob`, built for each placeholder | `ZONES_HOST` |
//! | `icu-cached` | the same, its provider and formatter kept per thread | `ZONES_HOST` |
//!
//! In UTC, no bidi isolation, as the native side formats the same catalogs.
//! `tools/e2e/datetime/web/speed.html` loads every build in one page and
//! alternates them, as the number probe does
//! (`bench/intl-probe/web/lib/speed.mjs`).

#[cfg(not(any(feature = "intl", feature = "icu")))]
compile_error!("build with exactly one of the features `intl`, `icu`, `icu-cached`");

use mf2::{
    Arg, BidiStrategy, Catalog, DateTime, FormatContext, FormatError, Formatter, Function, MsgId,
    NoErrors, Registry, TimeZone,
};
use wasm_bindgen::prelude::wasm_bindgen;

static FUNCTIONS: [(&str, &dyn Function); 3] = [
    ("date", &mf2_fn_datetime::DATE),
    ("datetime", &mf2_fn_datetime::DATETIME),
    ("time", &mf2_fn_datetime::TIME),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);

/// The host: the zone offsets (and, for `Intl`, the formatting) from the
/// browser's own data.
#[cfg(feature = "icu")]
fn context() -> FormatContext {
    let mut cx = FormatContext::new(&mf2::host_web::ZONES_HOST);
    cx.bidi = BidiStrategy::None;
    cx.time_zone = TimeZone::UTC;
    cx
}

/// The host: the zone offsets (and, for `Intl`, the formatting) from the
/// browser's own data.
#[cfg(not(feature = "icu"))]
fn context() -> FormatContext {
    let mut cx = FormatContext::new(&mf2::host_web::INTL_HOST);
    cx.bidi = BidiStrategy::None;
    cx.time_zone = TimeZone::UTC;
    cx
}

/// One message's catalog, as `MsgId` 0 (`mf2::Compiled::ID`, build side).
const ID: MsgId = MsgId::from_raw(0);

/// An error's suite name (`BadOperand` → `bad-operand`).
fn suite_name(e: FormatError) -> String {
    let mut s = String::new();
    for (i, c) in format!("{e:?}").chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                s.push('-');
            }
            s.push(c.to_ascii_lowercase());
        } else {
            s.push(c);
        }
    }
    s
}

/// The loaded catalogs and the argument values.
#[wasm_bindgen]
pub struct Bench {
    catalogs: Vec<Catalog>,
    values: Vec<DateTime<'static>>,
}

#[wasm_bindgen]
impl Bench {
    /// `values`: a JSON array of date/time literals (speed.json's
    /// `values`); one that does not parse is left out.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(values: &str) -> Bench {
        let values: Vec<String> = serde_json::from_str(values).unwrap_or_default();
        Bench {
            catalogs: Vec::new(),
            values: values
                .iter()
                .filter_map(|v| mf2_fn_datetime::parse_literal(v))
                .collect(),
        }
    }

    /// How many values parsed (the page checks it against speed.json).
    #[must_use]
    pub fn value_count(&self) -> u32 {
        u32::try_from(self.values.len()).unwrap_or(0)
    }

    /// Loads a catalog (manifest hash `hash`, decimal): `""`, or why not.
    pub fn load(&mut self, catalog: Vec<u8>, hash: &str) -> String {
        let Ok(hash) = hash.parse::<u64>() else {
            return "ERROR hash".into();
        };
        match Catalog::new(catalog, hash) {
            Ok(c) => {
                self.catalogs.push(c);
                String::new()
            }
            Err(e) => format!("ERROR {e:?}"),
        }
    }

    /// Catalog `cat` formatted with value `value`: its text, a tab, and its
    /// errors' suite names, comma-separated.
    #[must_use]
    pub fn sample(&self, cat: u32, value: u32) -> String {
        let (Some(c), Some(d)) = (
            self.catalogs.get(cat as usize),
            self.values.get(value as usize),
        ) else {
            return "\tmissing".into();
        };
        let cx = context();
        let f = Formatter::new(c, &REGISTRY, &cx);
        let mut text = String::new();
        let mut errors = Vec::new();
        f.write_named(ID, &[("d", Arg::DateTime(d))], &mut text, &mut errors);
        let errors: Vec<String> = errors.into_iter().map(suite_name).collect();
        format!("{text}\t{}", errors.join(","))
    }

    /// `iters` formats of catalog `cat`'s message, the argument cycling
    /// through the values; the total output length (keeps the work alive).
    #[must_use]
    pub fn bench(&self, cat: u32, iters: u32) -> u32 {
        let Some(c) = self.catalogs.get(cat as usize) else {
            return 0;
        };
        if self.values.is_empty() {
            return 0;
        }
        let cx = context();
        let f = Formatter::new(c, &REGISTRY, &cx);
        let mut out = String::with_capacity(256);
        let mut total = 0u32;
        let mut values = self.values.iter().cycle();
        for _ in 0..iters {
            let Some(d) = values.next() else {
                break;
            };
            out.clear();
            f.write_named(ID, &[("d", Arg::DateTime(d))], &mut out, &mut NoErrors);
            total = total.wrapping_add(u32::try_from(out.len()).unwrap_or(0));
        }
        total
    }
}

/// The backend the statics use, and the build's features (to show what
/// each build formats with).
#[wasm_bindgen]
#[must_use]
pub fn backend() -> String {
    let cache = if cfg!(feature = "icu-cached") {
        " (cache)"
    } else {
        ""
    };
    format!(
        "{}{cache}",
        core::any::type_name::<mf2_fn_datetime::DefaultBackend>()
    )
}
