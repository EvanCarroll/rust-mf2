//! Locale-output goldens (plans/01-conformance.md §5; plans/11 A8). The
//! suite leaves number and date output implementation-defined; the goldens
//! pin ours for the locale panel — a fixed message × argument set per
//! function family, per locale — formatted from `mf2::compile_str` catalogs
//! with the all-features L4 registry. `cargo xtask goldens` writes
//! `conformance/goldens/<family>.tsv` (reviewed, committed);
//! `tests/goldens.rs` requires the committed files to equal a fresh render,
//! and `cargo xtask l4-wasi` formats every golden case on `wasm32-wasip1` too,
//! so the Rust backends' output is identical on both targets.
//!
//! A file is a comment header, then one line per case:
//! `locale \t message \t argument \t output \t errors` — the argument as the
//! runner takes it, the output as a JSON string (bidi marks and spaces
//! visible as escapes), the errors' suite names.

use std::fmt::Write as _;

use mf2_l4_runner::{ArgSpec, Case, Config};
use mf2_runtime::{BidiStrategy, Date, DateTime, Time};

/// The locale panel of plans/01-conformance.md §5, and two tags with a
/// non-Latin numbering system.
pub const PANEL: [&str; 13] = [
    "en",
    "es",
    "de",
    "fr",
    "ar",
    "he",
    "ja",
    "hi",
    "ru",
    "pl",
    "cy",
    "ar-EG",
    "hi-u-nu-deva",
];

/// One function family's golden set.
#[derive(Debug, Clone, Copy)]
pub struct Family {
    /// The file stem under `conformance/goldens/`.
    pub name: &'static str,
    /// What the family covers (the file's header).
    pub about: &'static str,
    /// The messages; each takes the one argument `$n`.
    pub messages: &'static [&'static str],
    /// The values of `$n`.
    pub values: fn() -> Vec<ArgSpec>,
}

const PLURAL_KEYS: &str =
    "zero {{zero}} one {{one}} two {{two}} few {{few}} many {{many}} * {{other}}";

fn number_values() -> Vec<ArgSpec> {
    vec![
        ArgSpec::Int(0),
        ArgSpec::Int(1),
        ArgSpec::Int(-1),
        ArgSpec::Int(2),
        ArgSpec::Int(5),
        ArgSpec::Int(21),
        ArgSpec::Int(1000),
        ArgSpec::Float(1.5),
        ArgSpec::Decimal("1234.5".to_owned()),
        ArgSpec::Decimal("-1234567.891".to_owned()),
        ArgSpec::Decimal("0.001".to_owned()),
        ArgSpec::Decimal("12345678901234567890".to_owned()),
    ]
}

fn measure_values() -> Vec<ArgSpec> {
    vec![
        ArgSpec::Int(0),
        ArgSpec::Int(1),
        ArgSpec::Int(-1),
        ArgSpec::Int(2),
        ArgSpec::Int(5),
        ArgSpec::Int(21),
        ArgSpec::Float(1.5),
        ArgSpec::Decimal("1234.5".to_owned()),
        ArgSpec::Decimal("-1234567.891".to_owned()),
    ]
}

fn date_values() -> Vec<ArgSpec> {
    let bce = Date::new(-44, 3, 15).map(|d| DateTime::floating(d, Time::MIDNIGHT));
    vec![
        // Instants (UTC): a winter afternoon, a summer night that is the
        // next day east of UTC, the epoch.
        ArgSpec::date_time("2006-01-02T15:04:06Z"),
        ArgSpec::date_time("2024-07-15T23:59:59Z"),
        ArgSpec::date_time("1970-01-01T00:00:00Z"),
        // An instant with its own offset.
        ArgSpec::date_time("2006-01-02T15:04:06+05:30"),
        // Floating values: a wall time, a leap day at midnight, and a day
        // before the common era.
        ArgSpec::date_time("2006-01-02T15:04:06"),
        ArgSpec::date_time("2000-02-29T00:00:00"),
        bce.map_or(ArgSpec::Other, ArgSpec::DateTime),
    ]
}

/// The families with goldens: numbers (Phase 4, A3), currencies and units
/// (A4), dates and times (A6).
pub const FAMILIES: &[Family] = &[
    Family {
        name: "numbers",
        about: "the decimal family (fn-number): unannotated numbers, :number with digit, grouping \
            and sign options, :integer, :percent, plural and ordinal selection",
        messages: &[
            "{$n}",
            "{$n :number}",
            "{$n :number minimumFractionDigits=2}",
            "{$n :number maximumSignificantDigits=3}",
            "{$n :number useGrouping=always}",
            "{$n :number useGrouping=never}",
            "{$n :number signDisplay=always}",
            "{$n :integer}",
            "{$n :percent}",
            "{$n :percent maximumFractionDigits=1}",
            ".input {$n :number} .match $n PLURAL",
            ".input {$n :number select=ordinal} .match $n PLURAL",
        ],
        values: number_values,
    },
    Family {
        name: "currency",
        about: "the currency family (fn-number): symbols, narrow symbols, codes, names by the \
                formatted number's plural category, accounting, no symbol, fraction digits",
        messages: &[
            "{$n :currency currency=EUR}",
            "{$n :currency currency=USD}",
            "{$n :currency currency=JPY}",
            "{$n :currency currency=CHF}",
            "{$n :currency currency=EUR currencyDisplay=code}",
            "{$n :currency currency=EUR currencyDisplay=name}",
            "{$n :currency currency=USD currencyDisplay=narrowSymbol}",
            "{$n :currency currency=USD currencySign=accounting}",
            "{$n :currency currency=EUR currencyDisplay=never}",
            "{$n :currency currency=EUR fractionDigits=0}",
        ],
        values: measure_values,
    },
    Family {
        name: "units",
        about: "the unit family (fn-number): short, narrow and long patterns by the formatted \
                number's plural category, compound units, one composed X-per-Y",
        messages: &[
            "{$n :unit unit=meter}",
            "{$n :unit unit=meter unitDisplay=long}",
            "{$n :unit unit=meter unitDisplay=narrow}",
            "{$n :unit unit=kilometer-per-hour}",
            "{$n :unit unit=kilometer-per-hour unitDisplay=long}",
            "{$n :unit unit=celsius}",
            "{$n :unit unit=kilogram unitDisplay=long}",
            "{$n :unit unit=liter-per-kilometer}",
            "{$n :unit unit=hour unitDisplay=long}",
        ],
        values: measure_values,
    },
    Family {
        name: "dates",
        about: "the date/time family (host-std-datetime-icu: ICU4X from the catalog's \
                icu.blob): :datetime, :date, :time with every length, the field sets, \
                precisions, hour12, zone styles in UTC, a named zone and an offset, two other \
                calendars; instants and floating values, in UTC",
        messages: &[
            "{$n :datetime}",
            "{$n :datetime dateLength=long timePrecision=second}",
            "{$n :datetime dateLength=short timePrecision=hour}",
            "{$n :date}",
            "{$n :date length=long}",
            "{$n :date length=short}",
            "{$n :date fields=year-month-day-weekday length=long}",
            "{$n :date fields=month-day}",
            "{$n :date fields=weekday length=long}",
            "{$n :time}",
            "{$n :time precision=second}",
            "{$n :time hour12=true}",
            "{$n :time hour12=false}",
            "{$n :datetime timeZone=UTC timeZoneStyle=short}",
            "{$n :datetime timeZone=|America/New_York| timeZoneStyle=long}",
            "{$n :time timeZone=|Asia/Kolkata| timeZoneStyle=short}",
            "{$n :datetime timeZone=|+05:45|}",
            "{$n :date calendar=japanese length=long}",
            "{$n :date calendar=hebrew}",
        ],
        values: date_values,
    },
];

/// A message with its `PLURAL` placeholder expanded.
fn source(message: &str) -> String {
    message.replace("PLURAL", PLURAL_KEYS)
}

/// One golden case.
#[derive(Debug, Clone)]
pub struct GoldenCase {
    pub locale: &'static str,
    pub message: String,
    pub arg: ArgSpec,
    pub case: Case,
}

/// Every case of `family`, in file order: locale, message, value.
pub fn cases(family: &Family) -> Result<Vec<GoldenCase>, String> {
    let mut out = Vec::new();
    for locale in PANEL {
        for message in family.messages {
            let src = source(message);
            let compiled = mf2::compile_str(&src, locale)
                .map_err(|e| format!("{locale} {src}: {e} {:?}", e.kinds()))?;
            let bytes = compiled.catalog.into_bytes();
            for arg in (family.values)() {
                out.push(GoldenCase {
                    locale,
                    message: src.clone(),
                    arg: arg.clone(),
                    case: Case {
                        id: format!("golden/{}/{locale}", family.name),
                        catalog: bytes.clone(),
                        manifest_hash: compiled.manifest.hash(),
                        bidi: BidiStrategy::None,
                        args: vec![("n".to_owned(), arg)],
                        config: Config::All,
                    },
                });
            }
        }
    }
    Ok(out)
}

/// The argument, as the golden file shows it.
fn show_arg(a: &ArgSpec) -> String {
    match a {
        ArgSpec::Str(s) => format!("str {s:?}"),
        ArgSpec::Int(n) => format!("int {n}"),
        ArgSpec::Float(x) => format!("float {x:?}"),
        ArgSpec::Decimal(d) => format!("decimal {d}"),
        ArgSpec::DateTime(d) => {
            let mut s = String::new();
            d.write_iso(&mut s);
            format!("datetime {s}")
        }
        ArgSpec::Other => "other".to_owned(),
    }
}

/// The golden file of `family`, freshly formatted.
pub fn render(family: &Family) -> Result<String, String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# rust-mf2 locale-output goldens: {} — {}.",
        family.name, family.about
    );
    out.push_str(
        "# Generated by `cargo xtask goldens` (conformance/src/goldens.rs); reviewed, committed, \
         checked by tests/goldens.rs and on wasm32-wasip1 by `cargo xtask l4-wasi`.\n",
    );
    out.push_str(
        "# CLDR 48.2.1. Line: locale \\t message \\t argument \\t output (JSON) \\t errors\n",
    );
    for g in cases(family)? {
        let r = mf2_l4_runner::run(&g.case)?;
        let text = show_text(&r.text);
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{text}\t{}",
            g.locale,
            g.message,
            show_arg(&g.arg),
            r.errors.join(",")
        );
    }
    Ok(out)
}

/// `text` as a JSON string in which the characters a reviewer cannot see
/// are escapes: controls, the no-break and other special spaces (U+00A0,
/// U+2000–U+200A, U+202F, U+205F, U+3000), the bidi marks and isolates
/// (U+061C, U+200E, U+200F, U+202A–U+202E, U+2066–U+2069), U+FEFF.
fn show_text(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0}'..='\u{1f}'
            | '\u{7f}'..='\u{a0}'
            | '\u{61c}'
            | '\u{2000}'..='\u{200f}'
            | '\u{2028}'..='\u{202f}'
            | '\u{205f}'..='\u{206f}'
            | '\u{3000}'
            | '\u{feff}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The golden file's path, relative to the repository root.
pub fn path(family: &Family) -> String {
    format!("conformance/goldens/{}.tsv", family.name)
}
