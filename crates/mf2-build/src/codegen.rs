//! The generated Rust module (`plans/05-tooling.md` §4,
//! `plans/19-native-and-terminal.md` §10).
//!
//! `build.rs` writes it to `OUT_DIR`; the i18n crate includes it at its root
//! with `mf2::include_generated!()`. Its text is the same in every build of
//! one corpus and emit mode: each compile-time choice goes through one of
//! `mf2`'s cfg-forwarding macros (`__mf2::__if_ssr! { … }`, A2), which keep
//! or drop what they are given by `mf2`'s own features, whichever crate
//! turned them on. It carries
//!
//! * `MANIFEST_HASH` and `SOURCE_LOCALE` — what the client (or the
//!   executable) and every catalog agree on (F6);
//! * `LOCALES` — the tags and their base direction;
//! * `LANGUAGE_MATCHING` — CLDR's language-matching data cut to those
//!   locales' languages (`mf2_locale_data::matching`): what the one matcher
//!   reads, and all of CLDR's table a browser's client carries (19 §9);
//! * `CATALOGS` — the catalogs themselves, **never in a browser's client**
//!   (`__if_host_std!`): keeping the names and hashes out of the client is
//!   what makes its wasm byte-identical across translation edits (P0.9). A
//!   module that embeds catalogs embeds each once, in one table that
//!   `CATALOGS` and `CORPUS` share (a server beside `native`);
//! * `registry()` — the closed world (B13): the handlers this corpus uses
//!   and no others, with `with_numbers` / `with_dates` only where a
//!   placeholder can actually receive one (#90);
//! * `host` — the host the corpus needs, so that a feature that is on but
//!   unused links none of its glue (B1′);
//! * `CORPUS` — for `mf2::native`: under [`Emit::Native`] /
//!   [`Emit::NativeFiles`] (each catalog's file name and, under `Native`,
//!   its bytes; nothing behind a mode), and under [`Emit::Both`] with
//!   `native`;
//! * `Locale` — one variant per locale, with `ALL`, `SOURCE`, `tag()`,
//!   `dir()`, `best_match()`, `FromStr` through the one matcher and
//!   `Display`; `format()` with `native`; `name()` when every locale has a
//!   `language.<tag>` message; a clap value parser with `clap`;
//! * `setup()`, `install()`, `install_from_directory()`, `set_locale()`,
//!   `preload_locale()`, `current_locale()` and `with_locale()`, each where
//!   the build has what it needs (19 §10's table), one item per
//!   combination of modes;
//! * `markup::*` with `ratatui`, and a `prelude`;
//! * `pub use ::mf2 as __mf2;` and the exported `tr!` wrapper, which bakes
//!   the manifest's absolute path and hash into every expansion — or, with
//!   [`crate::Build::manifest_inline`], the manifest's bytes, so that an
//!   expansion survives a target directory that moved (D8).
//!
//! A native module ([`Emit::Native`], [`Emit::NativeFiles`]) never mentions
//! the browser's build in its documentation. The file is regenerated
//! whenever the corpus or the feature set changes, and written only when
//! its bytes differ.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use mf2_catalog::Dir;

use crate::build::{Emit, LocaleInfo};
use crate::error::{Error, Result};
use crate::features::Features;

/// What the module is generated from.
#[derive(Debug)]
pub struct Module<'a> {
    /// The crate path re-exported as `__mf2` (`::mf2`).
    pub facade: &'a str,
    /// The absolute path of `manifest.mf2m`, baked into `tr!`.
    pub manifest_path: &'a Path,
    /// `manifest_hash`.
    pub manifest_hash: u64,
    /// The source locale.
    pub source_locale: &'a str,
    /// Every locale, in tag order.
    pub locales: &'a [LocaleInfo],
    /// CLDR's language-matching data cut to the locales' languages, as a
    /// Rust expression of `mf2::LanguageMatching`
    /// (`mf2_locale_data::matching::Encoded::rust`).
    pub language_matching: &'a str,
    /// The function identifiers the corpus uses, ascending.
    pub functions: &'a [String],
    /// `mf2.toml`'s `[functions]`: identifier → Rust path.
    pub custom: &'a BTreeMap<String, String>,
    /// The client feature set.
    pub features: &'a Features,
    /// The corpus has a placeholder with no function, which can receive a
    /// number or a date at run time.
    pub unannotated: bool,
    /// How many messages, for the header comment.
    pub messages: usize,
    /// What this build writes: with [`Emit::Module`] the catalogs are
    /// another crate's, and nothing here names one.
    pub emit: Emit,
    /// The manifest's bytes, when the wrapper bakes them in instead of the
    /// path ([`crate::Build::manifest_inline`]): every expansion is then
    /// independent of where the target directory lives.
    pub manifest_bytes: Option<&'a [u8]>,
    /// The markup names the corpus uses, ascending: `markup::*`.
    pub markup: &'a [String],
    /// For each of `locales`, whether it has a `language.<tag>` message with
    /// no argument: the switcher's names, and `Locale::name()` when every
    /// locale has one.
    pub names: &'a [bool],
}

/// Refuses what would make the generated module ambiguous: two tags that
/// give `Locale` one variant, and two markup names with one hash (which a
/// style could not tell apart, as `tr!` refuses within one message).
pub(crate) fn check(m: &Module<'_>) -> Result<()> {
    let mut variants: BTreeMap<String, &str> = BTreeMap::new();
    for locale in m.locales {
        let name = variant(&locale.tag);
        if let Some(first) = variants.insert(name.clone(), &locale.tag) {
            return Err(Error::LocaleVariant {
                first: first.to_owned(),
                second: locale.tag.clone(),
                variant: name,
            });
        }
    }
    let mut keys: BTreeMap<u64, &str> = BTreeMap::new();
    for name in m.markup {
        if let Some(first) = keys.insert(mf2_catalog::markup_key(name), name) {
            return Err(Error::MarkupHash {
                first: first.to_owned(),
                second: name.clone(),
            });
        }
    }
    Ok(())
}

/// A tag's variant of `Locale`: its subtags in upper camel case (`pt-BR` is
/// `PtBr`, `es-419` is `Es419`).
pub(crate) fn variant(tag: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    for part in tag.split(['-', '_']) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.extend(chars.flat_map(char::to_lowercase));
        }
    }
    out
}

/// A markup name's constant in `markup`: its ASCII letters and digits in
/// upper case, and `_` for every other character (`key-name` and
/// `ns:key.name` give `KEY_NAME` and `NS_KEY_NAME`).
pub(crate) fn constant(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// The markup names that get a constant, with it. Every markup name MF2
/// allows is valid in a corpus, so none is refused: a name whose constant
/// has no letter or digit (`+:_¡`), or shares its constant with another
/// name (`a-b`, `a.b`), gets none.
fn constants(names: &[String]) -> Vec<(&str, String)> {
    let all: Vec<(&str, String)> = names.iter().map(|n| (n.as_str(), constant(n))).collect();
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, c) in &all {
        *count.entry(c.as_str()).or_default() += 1;
    }
    let keep: Vec<bool> = all
        .iter()
        .map(|(_, c)| {
            count.get(c.as_str()) == Some(&1)
                && c.bytes().any(|b| b.is_ascii_alphanumeric())
                && !c.starts_with(|d: char| d.is_ascii_digit())
        })
        .collect();
    all.into_iter()
        .zip(keep)
        .filter_map(|(pair, keep)| keep.then_some(pair))
        .collect()
}

/// The module's source.
pub fn write(module: &Module<'_>) -> String {
    let mut s = String::with_capacity(8192);
    header(&mut s, module);
    identity(&mut s, module);
    locales(&mut s, module);
    if embeds(module.emit) {
        embedded(&mut s, module);
    }
    registry(&mut s, module);
    if is_native(module.emit) {
        native_host(&mut s, module);
    } else {
        host(&mut s, module);
    }
    if has_corpus(module.emit) {
        corpus(&mut s, module);
    }
    locale(&mut s, module);
    functions(&mut s, module);
    markup(&mut s, module);
    tr(&mut s, module);
    if module.names.contains(&true) {
        names(&mut s, module);
    }
    prelude(&mut s, module);
    s
}

/// Whether `emit` builds for a native application.
pub(crate) const fn is_native(emit: Emit) -> bool {
    matches!(emit, Emit::Native | Emit::NativeFiles)
}

/// Whether the module embeds the catalogs' bytes.
const fn embeds(emit: Emit) -> bool {
    matches!(emit, Emit::Both | Emit::Native)
}

/// Whether the module has a `CORPUS` (under [`Emit::Both`], with `native`).
const fn has_corpus(emit: Emit) -> bool {
    matches!(emit, Emit::Both | Emit::Native | Emit::NativeFiles)
}

/// The catalog table on its own, for a crate only the server binary depends
/// on ([`Emit::Catalogs`]).
///
/// The whole file is server-side by construction, so nothing in it is behind
/// a mode; the crate that includes it is the gate.
pub fn write_catalogs(module: &Module<'_>) -> String {
    let mut s = String::with_capacity(512);
    header(&mut s, module);
    let _ = write!(
        s,
        "
/// The facade the embedded catalogs are read through.
#[doc(hidden)]
pub use {facade} as __mf2;
",
        facade = module.facade
    );
    s.push('\n');
    s.push_str(&catalogs(module, None));
    s
}

fn header(s: &mut String, m: &Module<'_>) {
    let _ = writeln!(
        s,
        "// @generated by mf2-build from locales/ — do not edit.\n\
         // {} messages, {} locale{}, {} function{}; features: {}.",
        m.messages,
        m.locales.len(),
        plural(m.locales.len()),
        m.functions.len(),
        plural(m.functions.len()),
        if m.features.names().next().is_none() {
            "(none)".to_owned()
        } else {
            m.features.names().collect::<Vec<_>>().join(", ")
        }
    );
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// `body`, each line indented, inside `__mf2::{name}! { … }`: kept or
/// dropped by `mf2`'s features.
fn gate(s: &mut String, name: &str, body: &str) {
    let _ = writeln!(s, "__mf2::{name}! {{");
    indent(s, body, "    ");
    s.push_str("}\n");
}

fn indent(s: &mut String, body: &str, by: &str) {
    for line in body.lines() {
        if !line.is_empty() {
            s.push_str(by);
            s.push_str(line);
        }
        s.push('\n');
    }
}

/// `0x43e0_dc12_eeb0_5ef1`: grouped, so that the generated file reads the way
/// a hand-written one would and clippy's pedantic lints stay quiet in the
/// crate that includes it.
fn hex_u64(value: u64) -> String {
    let hex = format!("{value:016x}");
    let mut out = String::from("0x");
    for (i, chunk) in hex.as_bytes().chunks(4).enumerate() {
        if i > 0 {
            out.push('_');
        }
        out.push_str(core::str::from_utf8(chunk).unwrap_or("0000"));
    }
    out
}

fn identity(s: &mut String, m: &Module<'_>) {
    let agree = if is_native(m.emit) {
        "the ids, slots, markup\n/// names and functions the executable and every catalog agree on. A catalog\n/// whose header carries another one is refused."
    } else {
        "the ids, slots, markup\n/// names and functions the wasm and every catalog agree on. A catalog whose\n/// header carries another one is rejected and refetched (F6)."
    };
    let _ = write!(
        s,
        "
/// The facade every `tr!` expansion goes through.
#[doc(hidden)]
pub use {facade} as __mf2;

/// `manifest_hash` (`plans/02-catalog-format.md` §3): {agree}
pub const MANIFEST_HASH: u64 = {hash};

/// The locale the manifest was built from.
pub const SOURCE_LOCALE: &str = {source:?};
",
        facade = m.facade,
        hash = hex_u64(m.manifest_hash),
        source = m.source_locale
    );
}

fn locales(s: &mut String, m: &Module<'_>) {
    let native = is_native(m.emit);
    let _ = write!(
        s,
        "
/// Every locale this corpus was built for, with its base direction, in tag
/// order. {what}
pub static LOCALES: &[(&str, __mf2::Dir)] = &[
",
        what = if native {
            "`Locale::ALL` is in the same order."
        } else {
            "No catalog name and no hash: the client is told which\n/// URL to fetch."
        }
    );
    for locale in m.locales {
        let _ = writeln!(
            s,
            "    ({:?}, __mf2::Dir::{}),",
            locale.tag,
            match locale.dir {
                Dir::Rtl => "Rtl",
                _ => "Ltr",
            }
        );
    }
    let who = if native {
        "It is what `CORPUS` matches\n/// the system's preferred languages with, and `Locale` a tag."
    } else {
        "It is all a browser's client carries of that\n/// table: a client-only application's boot matches the reader's languages\n/// with it (`install()` passes it), and a hydrated page never matches."
    };
    let _ = write!(
        s,
        "];

/// Whether `tag` is one of them.
pub fn has_locale(tag: &str) -> bool {{
    LOCALES.iter().any(|(t, _)| *t == tag)
}}

/// CLDR's language-matching data, cut to these locales' languages: the rules
/// that can serve one of them, and the likely subtags of the languages whose
/// readers those rules accept. For every reader it gives the answers CLDR's
/// whole table gives. {who}
pub static LANGUAGE_MATCHING: __mf2::LanguageMatching = {matching};
",
        matching = m.language_matching
    );
}

/// The catalogs' bytes, embedded once, and the tables that share them:
/// `CATALOGS` for a server (never a browser's client) and, with `native`,
/// `CORPUS` (written by [`corpus`]).
fn embedded(s: &mut String, m: &Module<'_>) {
    let native = is_native(m.emit);
    let mut table = String::new();
    let _ = writeln!(
        table,
        "/// The catalogs' bytes, embedded once: what {} share.\nstatic MF2_CATALOG_BYTES: [&[u8]; {}] = [",
        if native {
            "`CORPUS` and, beside `ssr`,\n/// `CATALOGS`"
        } else {
            "`CATALOGS` and, with `native`,\n/// `CORPUS`"
        },
        m.locales.len()
    );
    for locale in m.locales {
        let _ = writeln!(
            table,
            "    include_bytes!(concat!(env!(\"OUT_DIR\"), \"/\", {:?})),",
            locale.file_name
        );
    }
    table.push_str("];\n");
    if has_server_data(m) {
        let _ = writeln!(
            table,
            "\n/// The server-only tables (`plan/08` §4.2), embedded once beside the\n/// catalogs: the LOCALE entries only native code reads, never published.\nstatic MF2_SERVER_DATA: [&[u8]; {}] = [",
            m.locales.len()
        );
        for locale in m.locales {
            let _ = writeln!(table, "    {},", server_bytes(locale));
        }
        table.push_str("];\n");
    }
    let catalogs = catalogs(m, Some("MF2_CATALOG_BYTES"));
    s.push('\n');
    if native {
        s.push_str(&table);
        s.push('\n');
        gate(s, "__if_ssr", &catalogs);
    } else {
        table.push('\n');
        table.push_str(&catalogs);
        gate(s, "__if_host_std", &table);
    }
}

/// Whether any locale has a server-only table (`plan/08` §4.2).
fn has_server_data(m: &Module<'_>) -> bool {
    m.locales.iter().any(|l| l.server_file_name.is_some())
}

/// A locale's server-only table as an expression: included from the output
/// directory, or empty.
fn server_bytes(locale: &LocaleInfo) -> String {
    match &locale.server_file_name {
        Some(name) => format!("include_bytes!(concat!(env!(\"OUT_DIR\"), \"/\", {name:?}))"),
        None => "&[]".to_owned(),
    }
}

/// `CATALOGS`, `catalog()` and `catalog_name()`: each catalog's bytes from
/// `table`, or included here ([`Emit::Catalogs`]), with its server-only
/// table from `MF2_SERVER_DATA` beside `table`, or included here.
fn catalogs(m: &Module<'_>, table: Option<&str>) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "/// {}\npub static CATALOGS: &[(&str, &str, &[u8], &[u8])] = &[",
        if is_native(m.emit) {
            "The catalogs, for the Leptos layer's server beside `native`: `(tag,\n/// file name, bytes, server-only table)`, the bytes `CORPUS` holds."
        } else {
            "The catalogs, embedded for the **server** only: `(tag, file name,\n/// bytes, server-only table)`. The client fetches its one locale instead, so\n/// its wasm holds no message text, no id and no catalog name (B6). The\n/// server-only table holds the locale data only the server reads; it is\n/// served to no one, and empty when there is none."
        }
    );
    let shared = table.is_some() && has_server_data(m);
    for (i, locale) in m.locales.iter().enumerate() {
        let bytes = match table {
            Some(table) => format!("{table}[{i}]"),
            None => format!(
                "include_bytes!(concat!(env!(\"OUT_DIR\"), \"/\", {:?}))",
                locale.file_name
            ),
        };
        let server = if shared {
            format!("MF2_SERVER_DATA[{i}]")
        } else {
            server_bytes(locale)
        };
        let _ = writeln!(
            s,
            "    ({tag:?}, {file:?}, {bytes}, {server}),",
            tag = locale.tag,
            file = locale.file_name
        );
    }
    s.push_str(
        "];

/// The embedded catalog of `tag`, for a server that serves it from memory.
pub fn catalog(tag: &str) -> Option<&'static [u8]> {
    CATALOGS.iter().find(|(t, ..)| *t == tag).map(|(_, _, b, _)| *b)
}

/// The file name `tag`'s catalog is published under, content-hashed and
/// served immutable.
pub fn catalog_name(tag: &str) -> Option<&'static str> {
    CATALOGS.iter().find(|(t, ..)| *t == tag).map(|(_, n, ..)| *n)
}
",
    );
    s
}

/// The Rust path of the handler for a built-in function under `features`.
fn builtin_path(name: &str, features: &Features) -> Option<&'static str> {
    Some(match (name, features.fn_number()) {
        ("string", _) => "__mf2::functions::STRING",
        ("number", false) => "__mf2::functions::NUMBER",
        ("integer", false) => "__mf2::functions::INTEGER",
        ("offset", false) => "__mf2::functions::OFFSET",
        ("number", true) => "__mf2::fn_number::NUMBER",
        ("integer", true) => "__mf2::fn_number::INTEGER",
        ("offset", true) => "__mf2::fn_number::OFFSET",
        ("percent", _) => "__mf2::fn_number::PERCENT",
        ("currency", _) => "__mf2::fn_number::CURRENCY",
        ("unit", _) => "__mf2::fn_number::UNIT",
        ("datetime", _) => "__mf2::fn_datetime::DATETIME",
        ("date", _) => "__mf2::fn_datetime::DATE",
        ("time", _) => "__mf2::fn_datetime::TIME",
        _ => return None,
    })
}

fn registry(s: &mut String, m: &Module<'_>) {
    let mut entries: Vec<(String, String)> = Vec::new();
    for name in m.functions {
        let path = match builtin_path(name, m.features) {
            Some(path) => path.to_owned(),
            // A custom function. `unknown-function` is an error by default,
            // so reaching here means the corpus turned that lint down: the
            // call stays in the catalog and the runtime reports Unknown
            // Function against it, which is the fallback the spec defines.
            None => match m.custom.get(name) {
                Some(path) => path.clone(),
                None => continue,
            },
        };
        entries.push((name.clone(), path));
    }
    let _ = write!(
        s,
        "
/// The handlers this corpus uses — and no others, so an unused function is
/// not linked (B13).
static FUNCTIONS: [(&str, &dyn __mf2::Function); {n}] = [
",
        n = entries.len()
    );
    for (name, path) in &entries {
        let _ = writeln!(s, "    ({name:?}, &{path}),");
    }
    let _ = write!(
        s,
        "];\n\nstatic REGISTRY: __mf2::Registry = __mf2::Registry::new(&FUNCTIONS)"
    );
    // The unannotated hooks: only with their feature, and only if some
    // placeholder can reach them (#90).
    if m.unannotated && m.features.fn_number() {
        let _ = write!(s, "\n    .with_numbers(&__mf2::fn_number::NUMBERS)");
    }
    if m.unannotated && m.features.fn_datetime() {
        let _ = write!(s, "\n    .with_dates(&__mf2::fn_datetime::DATES)");
    }
    let _ = write!(
        s,
        ";

/// The registry every formatter in this application uses.
pub fn registry() -> &'static __mf2::Registry {{
    &REGISTRY
}}
"
    );
}

/// Whether a date can reach this corpus's messages: a date function, or a
/// placeholder with no function, which may be handed one at run time. The
/// date host is named only then (`plan/01` §4.1).
fn reaches_a_date(m: &Module<'_>) -> bool {
    m.features.fn_datetime()
        && (m.unannotated
            || m.functions
                .iter()
                .any(|f| matches!(f.as_str(), "datetime" | "date" | "time")))
}

/// Whether `number-intl` is on and a number can reach this corpus's messages: a
/// numeric function, or a placeholder with no function. In a browser the
/// host that answers `Host::numbers` with `Intl` is named only then
/// (`plan/01` §8 F1).
fn reaches_a_number(m: &Module<'_>) -> bool {
    m.features.number_intl()
        && (m.unannotated
            || m.functions.iter().any(|f| {
                matches!(
                    f.as_str(),
                    "number" | "integer" | "offset" | "math" | "percent" | "currency" | "unit"
                )
            }))
}

/// `__use_host!`'s input: `dates` and `numbers`, each when the corpus can
/// reach one.
fn host_input(m: &Module<'_>) -> &'static str {
    match (reaches_a_date(m), reaches_a_number(m)) {
        (true, true) => "dates numbers",
        (true, false) => "dates",
        (false, true) => "numbers",
        (false, false) => "",
    }
}

fn host(s: &mut String, m: &Module<'_>) {
    // Which host depends on how `mf2` was built, which is not the build's
    // to know: `__use_host!` is defined once per combination of `mf2`'s
    // features, and names one static, so the others are never linked
    // (B1′). A corpus with no date names no date host at all, and one with
    // no number no `Intl` number host.
    let dates = host_input(m);
    let _ = write!(
        s,
        "
/// The host this build formats through: the server's, or the browser's — and
/// of the browser's, the one this corpus actually needs, so that the glue of
/// a feature that is on but unused is never linked (B1′).
pub mod host {{
    super::__mf2::__use_host!({dates});
}}
"
    );
}

fn native_host(s: &mut String, m: &Module<'_>) {
    let dates = host_input(m);
    let _ = write!(
        s,
        "
/// The host this build formats through: the native one — for a corpus a
/// date can reach, the one that resolves a named time zone, so that nothing
/// else links a time-zone database (B1′).
pub mod host {{
    super::__mf2::__use_host!({dates});
}}
"
    );
}

fn corpus(s: &mut String, m: &Module<'_>) {
    let mut c = String::new();
    let _ = writeln!(
        c,
        "/// Everything `mf2::native` needs, as one value: the source locale, the
/// manifest hash, the locales, the registry, each catalog's file name{embedded},
/// and the part of CLDR's language-matching data they need.
pub static CORPUS: __mf2::Corpus = __mf2::Corpus::new(
    SOURCE_LOCALE,
    MANIFEST_HASH,
    LOCALES,
    &REGISTRY,
    &[",
        embedded = if embeds(m.emit) { " and bytes" } else { "" }
    );
    for (i, locale) in m.locales.iter().enumerate() {
        let bytes = if embeds(m.emit) {
            format!("Some(MF2_CATALOG_BYTES[{i}])")
        } else {
            "None".to_owned()
        };
        // The server-only table, shared with `CATALOGS` (only a build that
        // embeds the catalogs for a browser's server writes one).
        let server = if embeds(m.emit) && locale.server_file_name.is_some() {
            format!(".with_server_data(MF2_SERVER_DATA[{i}])")
        } else {
            String::new()
        };
        let _ = writeln!(
            c,
            "        __mf2::CatalogFile::new({tag:?}, {file:?}, {bytes}){server},",
            tag = locale.tag,
            file = locale.file_name
        );
    }
    c.push_str(
        "    ],\n)\n.with_language_matching(&LANGUAGE_MATCHING)\n.with_host(&host::HOST);\n",
    );
    s.push('\n');
    if is_native(m.emit) {
        s.push_str(&c);
    } else {
        // What `Locale::format` reads: with `native`, and on a server.
        gate(s, "__if_format", &c);
    }
}

fn locale(s: &mut String, m: &Module<'_>) {
    let variants: Vec<(String, &LocaleInfo)> =
        m.locales.iter().map(|l| (variant(&l.tag), l)).collect();
    let source = variant(m.source_locale);
    let hydrated = if is_native(m.emit) {
        ""
    } else {
        " In a hydrated page, which never matches (the\n///   server chose), `from_str` takes an exact tag."
    };
    let _ = write!(
        s,
        "
/// The languages this application is translated into: one variant per
/// locale, in tag order (`pt-BR` is `PtBr`).
///
/// * `ALL` lists them, and `SOURCE` is the one the messages are written in;
/// * `tag()` and `dir()` give the tag and its base direction, and `Display`
///   writes the tag;
/// * `from_str` (`\"fr_CA.UTF-8\".parse()`) and `best_match` choose the
///   language that best serves a reader, by CLDR's language-matching data;
///   `from_str`'s error lists the languages there are.{hydrated}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Locale {{
"
    );
    for (name, l) in &variants {
        let _ = writeln!(s, "    /// `{}`\n    {name},", l.tag);
    }
    let _ = write!(
        s,
        "}}

impl Locale {{
    /// Every language, in tag order, as `LOCALES` lists them.
    pub const ALL: [Locale; {n}] = [{all}];

    /// The language the messages are written in.
    pub const SOURCE: Locale = Locale::{source};

    /// The BCP 47 tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {{
        match self {{
",
        n = variants.len(),
        all = variants
            .iter()
            .map(|(name, _)| format!("Locale::{name}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    for (name, l) in &variants {
        let _ = writeln!(s, "            Locale::{name} => {:?},", l.tag);
    }
    let rtl: Vec<&str> = variants
        .iter()
        .filter(|(_, l)| l.dir == Dir::Rtl)
        .map(|(name, _)| name.as_str())
        .collect();
    let dir = if rtl.is_empty() {
        "        let _ = self;\n        __mf2::Dir::Ltr\n".to_owned()
    } else if rtl.len() == variants.len() {
        "        let _ = self;\n        __mf2::Dir::Rtl\n".to_owned()
    } else {
        format!(
            "        match self {{\n            {} => __mf2::Dir::Rtl,\n            _ => __mf2::Dir::Ltr,\n        }}\n",
            rtl.iter()
                .map(|name| format!("Locale::{name}"))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    let _ = write!(
        s,
        "        }}
    }}

    /// The base direction of the language's text.
    #[must_use]
    pub const fn dir(self) -> __mf2::Dir {{
{dir}    }}

    /// The language that best serves a reader of `desired`, their languages
    /// in order of preference, by CLDR's language-matching data; `None` when
    /// none is close enough (the source language is then the usual answer).
    pub fn best_match<'a>(desired: impl IntoIterator<Item = &'a str>) -> Option<Locale> {{
        LANGUAGE_MATCHING
            .best_match(desired, LOCALES)
            .and_then(|index| Locale::ALL.get(index).copied())
    }}
}}

/// The language that best serves `tag` (`fr_CA.UTF-8` is French); an
/// error, which lists the languages there are, when none is close enough.
impl ::core::str::FromStr for Locale {{
    type Err = __mf2::UnknownLocale;

    fn from_str(tag: &str) -> Result<Locale, __mf2::UnknownLocale> {{
        __mf2::__best_locale!(LANGUAGE_MATCHING, LOCALES, tag)
            .and_then(|index| Locale::ALL.get(index).copied())
            .ok_or(__mf2::UnknownLocale::new(LOCALES))
    }}
}}

/// The tag.
impl ::core::fmt::Display for Locale {{
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{
        f.write_str(self.tag())
    }}
}}

__mf2::__if_clap! {{
    /// `--lang fr_CA.UTF-8` parses through `from_str`, and `--help` lists the
    /// tags.
    impl __mf2::__generated::clap::ValueParserFactory for Locale {{
        type Parser = __mf2::__generated::clap::LocaleParser<Locale>;

        fn value_parser() -> Self::Parser {{
            __mf2::__generated::clap::LocaleParser::new(
                LOCALES,
                <Locale as ::core::str::FromStr>::from_str,
            )
        }}
    }}
}}
"
    );
    if has_corpus(m.emit) {
        s.push('\n');
        gate(
            s,
            if is_native(m.emit) {
                "__if_native"
            } else {
                "__if_format"
            },
            "impl Locale {
    /// `message`, formatted in this language. It needs no `install()`, and
    /// chooses no app-wide language.
    #[must_use]
    pub fn format(self, message: &impl __mf2::Message) -> __mf2::__generated::String {
        __mf2::__generated::format_in(&CORPUS, self.tag(), message)
    }
}
",
        );
    }
    s.push('\n');
    gate(
        s,
        "__if_axum",
        "/// An Axum extractor: the language the `Negotiator` layer chose for this
/// request, else the one `mf2::axum::Negotiator::default()`'s sources
/// negotiate. It never rejects a request.
impl<S: ::core::marker::Send + ::core::marker::Sync> __mf2::__generated::axum::FromRequestParts<S>
    for Locale
{
    type Rejection = ::core::convert::Infallible;

    fn from_request_parts(
        parts: &mut __mf2::__generated::axum::Parts,
        _state: &S,
    ) -> impl ::core::future::Future<Output = ::core::result::Result<Self, Self::Rejection>>
           + ::core::marker::Send {
        let locale = __mf2::__generated::axum::locale_index(parts, LOCALES, SOURCE_LOCALE)
            .and_then(|index| Locale::ALL.get(index).copied())
            .unwrap_or(Locale::SOURCE);
        ::core::future::ready(::core::result::Result::Ok(locale))
    }
}
",
    );
}

/// The locale functions, each once, whatever the combination of modes:
/// each body names every mode's work through the cfg-forwarding macros.
fn functions(s: &mut String, m: &Module<'_>) {
    let native = is_native(m.emit);
    // What the Leptos layer is given: a client-only application's boot
    // matches the reader's languages, so its setup alone carries the
    // matching data (a hydrated page never matches, and links none).
    s.push('\n');
    gate(
        s,
        "__if_leptos",
        &"/// What the Leptos layer is given, from what this build generated: the
/// registry, the host, the manifest hash and the locales, and a client-only
/// application's language-matching data. `install()` installs it; an
/// application that adds to it (`setup().with_time_zone(…)`) installs it
/// itself, once on each side.
#[must_use]
#[allow(clippy::let_and_return)]
pub fn setup() -> __mf2::leptos::Setup {
    let setup = __mf2::leptos::Setup::new(
        registry(),
        &host::HOST,
        MANIFEST_HASH,
        SOURCE_LOCALE,
        LOCALES,
    );
    __mf2::__if_csr! {
        let setup = setup.with_language_matching(&LANGUAGE_MATCHING);
    }
    __NAMES__setup
}
"
        .replace(
            "__NAMES__",
            if m.names.contains(&true) {
                "let setup = setup.with_names(__locale_name);\n    "
            } else {
                ""
            },
        ),
    );

    // `install()`.
    let mut body = String::new();
    if embeds(m.emit) {
        body.push_str(
            "    __mf2::__if_native! {\n        __mf2::native::install(&CORPUS);\n    }\n",
        );
        body.push_str(
            "    __mf2::__if_ssr! {\n        __mf2::__generated::install_server(setup(), CATALOGS);\n    }\n",
        );
        body.push_str(
            "    __mf2::__if_axum! {\n        __mf2::__generated::install_axum(&CORPUS);\n    }\n",
        );
    } else {
        body.push_str("    __mf2::__if_ssr! {\n        __mf2::leptos::install(setup());\n    }\n");
    }
    body.push_str("    __mf2::__if_client! {\n        __mf2::leptos::install(setup());\n    }\n");
    let doc = match m.emit {
        Emit::Native => {
            "/// Installs the catalogs the executable embeds as the process's, and makes
/// the language that best serves the system's preferred languages the
/// app-wide one, else the source language. Call it once, at start-up. It
/// returns nothing: embedded catalogs from the same build cannot fail to
/// load. Beside a Leptos mode it also gives the Leptos layer this build's
/// setup, and on the server the same catalogs."
        }
        Emit::NativeFiles => {
            "/// Gives the Leptos layer this build's setup (its catalogs are files:
/// `install_from_directory` installs them)."
        }
        Emit::Module => {
            "/// Gives the Leptos layer what this build generated, `setup()`. Call it
/// once on each side, before rendering or hydrating. This module names no
/// catalog: the server installs them from the crate that embeds them."
        }
        _ => {
            "/// Gives the Leptos layer what this build generated, `setup()`, and, on
/// the server, the embedded catalogs, each checked against the manifest
/// hash. Call it once on each side, before rendering or hydrating. With
/// `native`, it also installs the catalogs as the process's (`mf2::native`);
/// with `axum`, as the ones `mf2::axum` negotiates among and serves.
///
/// # Panics
///
/// On the server, if an embedded catalog does not load: a corrupt
/// executable."
        }
    };
    let install = format!("{doc}\npub fn install() {{\n{body}}}\n");
    s.push('\n');
    gate(
        s,
        if embeds(m.emit) {
            "__if_install"
        } else {
            "__if_leptos"
        },
        &install,
    );

    if has_corpus(m.emit) {
        s.push('\n');
        gate(
            s,
            "__if_native",
            "/// Installs the catalog files this build wrote from `directory` as the
/// process's, and chooses the app-wide language as `install()` does. Only
/// the source language's file is required; each must hash to its name.
pub fn install_from_directory(
    directory: impl AsRef<__mf2::__generated::Path>,
) -> Result<(), __mf2::native::Error> {
    __mf2::native::install_from_directory(&CORPUS, directory)
}
",
        );
    }

    // The locale functions exist with a Leptos mode, and with `native`
    // wherever there is a `CORPUS` to install.
    let mode = if has_corpus(m.emit) {
        "__if_mode"
    } else {
        "__if_leptos"
    };
    let set = if native {
        "/// Makes `locale` the app-wide language: every thread's next format uses
/// it, but a thread inside `with_locale`. Beside a Leptos mode, a client
/// switches its page (spawned), and the server does nothing.
///
/// # Panics
///
/// Before `install()`."
    } else {
        "/// Switches to `locale`: in the browser, fetch, check and swap the catalog,
/// update every live text and remember the choice (spawned; a failure
/// leaves the page as it is). On the server it does nothing: the request's
/// language is the negotiation's. With `native`, it sets the app-wide
/// language too."
    };
    let current = if native {
        "/// The language this thread formats in: its own (`with_locale`), else the
/// app-wide one. Beside a Leptos mode, the request's or the page's first.
///
/// # Panics
///
/// In a build whose only mode is `native`, before `install()` outside
/// `with_locale`."
    } else {
        "/// The language of the request being rendered, or of the page — tracked:
/// a view that reads it follows a switch. With `native`, then the native
/// store's; the source language before any is chosen."
    };
    let _ = write!(
        s,
        "
__mf2::{mode}! {{
{set_doc}
    pub fn set_locale(locale: Locale) {{
        __mf2::__generated::set_locale(locale.tag());
    }}

{current_doc}
    #[must_use]
    pub fn current_locale() -> Locale {{
        __mf2::__generated::current_locale(LOCALES)
            .and_then(|index| Locale::ALL.get(index).copied())
            .unwrap_or(Locale::SOURCE)
    }}
}}
",
        set_doc = indented(set),
        current_doc = indented(current),
    );

    s.push('\n');
    gate(
        s,
        "__if_leptos",
        "/// Fetches and checks `locale`'s catalog without switching to it — what a
/// language menu calls on hover (spawned). On the server it does nothing.
pub fn preload_locale(locale: Locale) {
    __mf2::__generated::preload_locale(locale.tag());
}

/// `<LocaleOption tag=Locale::Fr>`: the helper's tag type.
impl From<Locale> for __mf2::leptos::LocaleTag {
    fn from(locale: Locale) -> __mf2::leptos::LocaleTag {
        __mf2::leptos::LocaleTag(locale.tag())
    }
}
",
    );

    if has_corpus(m.emit) {
        s.push('\n');
        gate(
            s,
            "__if_native",
            "/// Runs `body` with this thread formatting in `locale`, and returns what it
/// returns; the thread's language is restored when `body` returns or
/// unwinds, and other threads are not affected. It needs no `install()`.
pub fn with_locale<R>(locale: Locale, body: impl FnOnce() -> R) -> R {
    __mf2::__generated::with_locale_in(&CORPUS, locale.tag(), body)
}
",
        );
    }
}

/// `text`, each line indented by four spaces.
fn indented(text: &str) -> String {
    let mut out = String::new();
    indent(&mut out, text, "    ");
    out.pop();
    out
}

/// `markup::*`, with `ratatui`: a constant per markup name, holding the
/// name and its hash.
fn markup(s: &mut String, m: &Module<'_>) {
    let mut body = String::from(
        "/// The markup names the messages use, one constant each, for a theme's
/// styles: `markup::KEY` is `{#key}…{/key}`. A name whose ASCII letters and
/// digits give no constant, or the same one as another name's, has none.
pub mod markup {
",
    );
    for (name, constant) in constants(m.markup) {
        let _ = writeln!(
            body,
            "    /// `{{#{name}}}`\n    pub const {constant}: super::__mf2::ratatui::Markup =\n        super::__mf2::ratatui::Markup::new({key}, {name:?});",
            key = hex_u64(mf2_catalog::markup_key(name)),
        );
    }
    body.push_str("}\n");
    s.push('\n');
    gate(s, "__if_ratatui", &body);
}

fn tr(s: &mut String, m: &Module<'_>) {
    // Either the manifest's absolute path, or — opt-in — its bytes, which
    // survive a target directory that moves (`Build::manifest_inline`).
    let (source, how) = match m.manifest_bytes {
        None => (
            format!("{:?}", m.manifest_path.display().to_string()),
            "The manifest's absolute path and its hash are baked in, so any crate that\n\
             /// depends on this one can call it",
        ),
        Some(bytes) => (
            format!("bytes {}", byte_string(bytes)),
            "The manifest's bytes and its hash are baked in (inline mode), so an\n\
             /// expansion needs no file and survives a target directory that moved",
        ),
    };
    let _ = write!(
        s,
        "
/// `tr!(\"id\", name = value, …)` — the id and the argument set checked
/// against the manifest at compile time.
///
/// {how}, and a manifest whose hash differs is
/// reported as stale rather than used (D8).
#[doc(hidden)]
#[macro_export]
macro_rules! __mf2_tr {{
    ($($t:tt)*) => {{
        $crate::__mf2::__tr_impl!({source} 0x{hash:016x}u64 ; $crate ; $($t)*)
    }};
}}
// Exported under a hidden name and named `tr` by a `use`, so that any module
// of this crate reaches it by path (`use crate::tr;`, the prelude), which a
// `macro_export` macro from an expansion cannot be (rust-lang/rust#52234).
#[allow(unused_imports)]
pub use __mf2_tr as tr;

/// `msg_id!(\"id\")` — the id checked against the manifest, and nothing
/// else: the `MsgId` a caller needs to format a message whose arguments are
/// not known until run time (`mf2::TrDyn`).
#[doc(hidden)]
#[macro_export]
macro_rules! __mf2_msg_id {{
    ($($t:tt)*) => {{
        $crate::__mf2::__msg_id_impl!({source} 0x{hash:016x}u64 ; $crate ; $($t)*)
    }};
}}
#[allow(unused_imports)]
pub use __mf2_msg_id as msg_id;
",
        hash = m.manifest_hash
    );
}

/// The id of the message that names the language `tag`: what the switcher
/// and `Locale::name()` show.
pub(crate) fn name_id(tag: &str) -> String {
    format!("language.{tag}")
}

/// For each of `tags`, whether `manifest` has its [`name_id`] with no
/// argument: [`Module::names`].
pub(crate) fn named(tags: &[&str], manifest: &mf2_catalog::Manifest) -> Vec<bool> {
    tags.iter()
        .map(|tag| {
            let id = name_id(tag);
            manifest
                .ids
                .iter()
                .position(|i| *i == id)
                .is_some_and(|at| manifest.slots.get(at).is_some_and(Vec::is_empty))
        })
        .collect()
}

/// The ids the generated module names itself, and no `tr!` of the
/// application's does: each named language's, for the switcher and
/// `Locale::name()`.
pub(crate) fn uses(tags: &[&str], manifest: &mf2_catalog::Manifest) -> Vec<String> {
    tags.iter()
        .zip(named(tags, manifest))
        .filter(|(_, named)| *named)
        .map(|(tag, _)| name_id(tag))
        .collect()
}

/// `Locale::name()`: after `tr!`, which it expands, as a macro is in scope
/// only after its definition.
fn names(s: &mut String, m: &Module<'_>) {
    if m.names.len() == m.locales.len() && m.names.iter().all(|named| *named) {
        let _ = write!(
            s,
            "
impl Locale {{
    /// The language's name, for a language menu: its `language.<tag>`
    /// message, which each translation writes.
    #[must_use]
    pub fn name(self) -> __mf2::Tr {{
        match self {{
"
        );
        for l in m.locales {
            let id = name_id(&l.tag);
            let _ = writeln!(s, "            Locale::{} => tr!({id:?}),", variant(&l.tag));
        }
        s.push_str("        }\n    }\n}\n");
    }
    // What `setup()` gives the switcher: each named language's message, by
    // its index in `LOCALES`.
    s.push_str(
        "
__mf2::__if_leptos! {
    /// The name of the language at `index` in `LOCALES`: `setup()`'s names.
    fn __locale_name(index: usize) -> Option<__mf2::Tr> {
        match index {
",
    );
    for (index, (l, named)) in m.locales.iter().zip(m.names).enumerate() {
        if *named {
            let id = name_id(&l.tag);
            let _ = writeln!(s, "            {index} => Some(tr!({id:?})),");
        }
    }
    s.push_str("            _ => None,\n        }\n    }\n}\n");
}

/// The prelude: `tr` and `msg_id`, `Locale`, the functions that choose and
/// read the language where this build has them, and the description types
/// for signatures. `install`, `setup` and `markup` stay out: each is named
/// once.
fn prelude(s: &mut String, m: &Module<'_>) {
    let mode = if has_corpus(m.emit) {
        "__if_mode"
    } else {
        "__if_leptos"
    };
    let _ = write!(
        s,
        "
/// What an application names everywhere, for `use …::prelude::*;` (in this
/// crate, `use crate::prelude::*;`): `tr!` and `msg_id!`, `Locale`, the
/// functions that choose and read the language where this build has them,
/// and the description types for signatures. `install`, `setup` and
/// `markup` stay out: each is named once.
pub mod prelude {{
    pub use super::{{Locale, msg_id, tr}};
    pub use super::__mf2::{{Tr, TrArgs, TrDyn, TrRich}};
    super::__mf2::{mode}! {{
        pub use super::{{current_locale, set_locale}};
    }}
    super::__mf2::__if_leptos! {{
        pub use super::preload_locale;
    }}
"
    );
    if has_corpus(m.emit) {
        s.push_str(
            "    super::__mf2::__if_native! {\n        pub use super::with_locale;\n    }\n",
        );
    }
    s.push_str("}\n");
}

/// `b"…"`: the bytes as a Rust byte-string literal, escaped so that the
/// generated module stays valid UTF-8 and diff-able.
fn byte_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2 + 3);
    out.push_str("b\"");
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\x{b:02x}");
            }
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::{Module, check, constant, variant, write};
    use crate::build::{Emit, LocaleInfo};
    use crate::error::Error;
    use crate::features::Features;
    use mf2_catalog::Dir;
    use std::collections::BTreeMap;
    use std::path::Path;

    fn locales() -> Vec<LocaleInfo> {
        vec![
            LocaleInfo {
                tag: "en".to_owned(),
                dir: Dir::Ltr,
                hash: "0123456789abcdef".to_owned(),
                file_name: "en.0123456789abcdef.mf2b".to_owned(),
                server_file_name: None,
            },
            LocaleInfo {
                tag: "ar".to_owned(),
                dir: Dir::Rtl,
                hash: "fedcba9876543210".to_owned(),
                file_name: "ar.fedcba9876543210.mf2b".to_owned(),
                server_file_name: None,
            },
        ]
    }

    fn module<'a>(
        functions: &'a [String],
        features: &'a Features,
        custom: &'a BTreeMap<String, String>,
        locales: &'a [LocaleInfo],
        unannotated: bool,
    ) -> Module<'a> {
        Module {
            facade: "::mf2",
            manifest_path: Path::new("/out/manifest.mf2m"),
            manifest_hash: 0x43e0_dc12_eeb0_5ef1,
            source_locale: "en",
            locales,
            functions,
            custom,
            features,
            unannotated,
            messages: 3,
            emit: Emit::Both,
            manifest_bytes: None,
            language_matching: "__mf2::LanguageMatching::EMPTY",
            markup: &[],
            names: &[],
        }
    }

    /// The text of the first `__mf2::{gate}! { … }` block at column 0.
    fn block<'c>(code: &'c str, gate: &str) -> &'c str {
        let open = format!("\n__mf2::{gate}! {{\n");
        let Some(start) = code.find(&open) else {
            return "";
        };
        let rest = &code[start + 1..];
        &rest[..rest.find("\n}\n").map_or(rest.len(), |end| end + 3)]
    }

    #[test]
    fn the_module_carries_the_corpus_cut_of_the_matching_data() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let cut = mf2_locale_data::matching::Matching::shipped()
            .expect("the shipped table")
            .cut(&["en", "ar"])
            .encode()
            .expect("encodes")
            .rust("__mf2::LanguageMatching");
        let mut m = module(&[], &features, &custom, &locales, false);
        m.language_matching = &cut;
        let code = write(&m);
        assert!(
            code.contains("pub static LANGUAGE_MATCHING: __mf2::LanguageMatching = __mf2::LanguageMatching::new(\n"),
            "{code}"
        );
        // Numbers only: no catalog name or hash, no text of the corpus.
        let start = code.find("pub static LANGUAGE_MATCHING").unwrap_or(0);
        let table = &code[start..];
        let table = &table[..table.find(");\n").unwrap_or(table.len())];
        assert!(!table.contains(".mf2b") && !table.contains('"'), "{table}");
        // English's and Arabic's paradigms and match variables, and the
        // languages whose readers accept either.
        assert!(table.lines().count() > 100, "{table}");
    }

    #[test]
    fn the_client_sees_no_catalog_name_or_hash() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let code = write(&module(&[], &features, &custom, &locales, false));
        // Everything that names a catalog is dropped from a browser's
        // client by `mf2`'s own features — the bytes, `CATALOGS` and, with
        // `native` or on a server, `CORPUS`.
        let server = block(&code, "__if_host_std");
        let format = block(&code, "__if_format");
        assert_eq!(server.matches(".mf2b").count(), 4, "{code}");
        assert_eq!(format.matches(".mf2b").count(), 2, "{code}");
        assert_eq!(code.matches(".mf2b").count(), 6, "{code}");
        assert!(server.contains("pub static CATALOGS:"), "{code}");
        assert!(format.contains("pub static CORPUS:"), "{code}");
        assert!(
            code.contains("pub const MANIFEST_HASH: u64 = 0x43e0_dc12_eeb0_5ef1;"),
            "{code}"
        );
        assert!(code.contains("pub static LOCALES: &[(&str, __mf2::Dir)]"));
        assert!(code.contains("(\"ar\", __mf2::Dir::Rtl),"));
    }

    // `plan/08` §4.2: the server-only table, embedded once for the server.
    #[test]
    fn the_server_only_table_is_embedded_once_for_the_server() {
        let mut locales = locales();
        locales[0].server_file_name = Some("en.0123456789abcdef.mf2b.server".to_owned());
        let features = Features::default();
        let custom = BTreeMap::new();
        let code = write(&module(&[], &features, &custom, &locales, false));
        let server = block(&code, "__if_host_std");
        let format = block(&code, "__if_format");
        // Behind the gate that keeps the catalogs out of a client, once.
        assert_eq!(code.matches(".mf2b.server").count(), 1, "{code}");
        assert_eq!(server.matches(".mf2b.server").count(), 1, "{code}");
        assert!(
            server.contains("static MF2_SERVER_DATA: [&[u8]; 2]"),
            "{code}"
        );
        // Arabic has none.
        assert!(server.contains("    &[],\n"), "{code}");
        // `CATALOGS` and `CORPUS` share it.
        assert!(
            server.contains("MF2_CATALOG_BYTES[0], MF2_SERVER_DATA[0]),"),
            "{code}"
        );
        assert!(
            format.contains(".with_server_data(MF2_SERVER_DATA[0]),"),
            "{code}"
        );
        assert!(!format.contains("MF2_SERVER_DATA[1]"), "{code}");

        // Without a table, `CATALOGS` carries an empty one.
        let plain = write(&module(&[], &features, &custom, &self::locales(), false));
        assert!(!plain.contains("MF2_SERVER_DATA"), "{plain}");
        assert!(plain.contains("MF2_CATALOG_BYTES[0], &[]),"), "{plain}");
    }

    #[test]
    fn a_native_module_embeds_each_catalog_once() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let mut m = module(&[], &features, &custom, &locales, false);

        m.emit = Emit::Native;
        let code = write(&m);
        assert!(!code.contains("host_web"), "{code}");
        assert!(code.contains("super::__mf2::__use_host!();"), "{code}");
        // One table of bytes: `CORPUS` always, and `CATALOGS` beside `ssr`.
        assert_eq!(code.matches("include_bytes!").count(), 2, "{code}");
        assert!(
            code.contains("\npub static CORPUS: __mf2::Corpus"),
            "{code}"
        );
        assert!(
            block(&code, "__if_ssr")
                .contains("    (\"ar\", \"ar.fedcba9876543210.mf2b\", MF2_CATALOG_BYTES[1]),"),
            "{code}"
        );
        // The corpus carries the cut its locales are matched with, and the
        // host this build formats it through.
        assert!(
            code.contains(
                "\n.with_language_matching(&LANGUAGE_MATCHING)\n.with_host(&host::HOST);"
            ),
            "{code}"
        );
        assert!(
            code.contains(
                "__mf2::CatalogFile::new(\"ar\", \"ar.fedcba9876543210.mf2b\", Some(MF2_CATALOG_BYTES[1])),"
            ),
            "{code}"
        );

        m.emit = Emit::NativeFiles;
        let code = write(&m);
        assert!(!code.contains("include_bytes!"), "{code}");
        assert!(!code.contains("CATALOGS"), "{code}");
        assert!(
            code.contains("__mf2::CatalogFile::new(\"en\", \"en.0123456789abcdef.mf2b\", None),"),
            "{code}"
        );
        assert!(code.contains("pub fn install_from_directory("), "{code}");
    }

    #[test]
    fn a_native_module_never_says_wasm() {
        let locales = locales();
        let features = Features::parse("fn-number,datetime");
        let custom = BTreeMap::new();
        let markup = ["key".to_owned()];
        let named = vec![true; locales.len()];
        for emit in [Emit::Native, Emit::NativeFiles, Emit::Both] {
            let mut m = module(&[], &features, &custom, &locales, true);
            m.emit = emit;
            m.markup = &markup;
            m.names = &named;
            let code = write(&m).to_ascii_lowercase();
            assert_eq!(
                code.contains("wasm"),
                emit == Emit::Both,
                "{emit:?}:\n{code}"
            );
        }
    }

    #[test]
    fn every_choice_is_mf2s_features_and_one_item_each() {
        let locales = locales();
        let features = Features::parse("fn-number,datetime,host-web-datetime-intl");
        let custom = BTreeMap::new();
        for emit in [Emit::Both, Emit::Module, Emit::Native, Emit::NativeFiles] {
            let mut m = module(&[], &features, &custom, &locales, false);
            m.emit = emit;
            let code = write(&m);
            // No `cfg` of the crate that includes it: that crate's
            // features say nothing about how `mf2` was built.
            assert!(!code.contains("#[cfg("), "{emit:?}:\n{code}");
            for item in [
                "pub fn setup()",
                "pub fn install()",
                "pub fn set_locale(",
                "pub fn current_locale(",
                "pub fn preload_locale(",
                "pub enum Locale",
                "FromRequestParts<S>",
            ] {
                assert_eq!(code.matches(item).count(), 1, "{emit:?} {item}:\n{code}");
            }
            let native = emit != Emit::Module;
            for item in [
                "pub fn with_locale<",
                "pub fn format(",
                "pub fn install_from_directory(",
            ] {
                assert_eq!(
                    code.matches(item).count(),
                    usize::from(native),
                    "{emit:?} {item}"
                );
            }
        }
    }

    #[test]
    fn a_web_module_has_a_corpus_only_where_it_formats() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let mut m = module(&[], &features, &custom, &locales, false);
        let code = write(&m);
        // With `native`, and on a server (`ssr`, `axum`): `Locale::format`.
        assert!(
            block(&code, "__if_format").contains("    pub static CORPUS:"),
            "{code}"
        );
        // Under `axum`, `Locale` is an extractor, and `install()` gives
        // `mf2::axum` the corpus.
        assert!(
            block(&code, "__if_axum").contains("FromRequestParts<S>"),
            "{code}"
        );
        assert!(code.contains("install_axum(&CORPUS)"), "{code}");
        assert_eq!(code.matches("pub static CORPUS:").count(), 1, "{code}");

        m.emit = Emit::Module;
        let code = write(&m);
        assert!(!code.contains("CORPUS"), "{code}");
        assert!(!code.contains("CatalogFile"), "{code}");
        assert!(!code.contains(".mf2b"), "{code}");
    }

    #[test]
    fn the_registry_is_closed_over_what_the_corpus_uses() {
        let locales = locales();
        let custom = BTreeMap::from([("app:emoji".to_owned(), "my_app::EMOJI".to_owned())]);
        let functions = ["integer".to_owned(), "app:emoji".to_owned()];

        let core = Features::default();
        let code = write(&module(&functions, &core, &custom, &locales, false));
        assert!(
            code.contains("(\"integer\", &__mf2::functions::INTEGER),"),
            "{code}"
        );
        assert!(code.contains("(\"app:emoji\", &my_app::EMOJI),"), "{code}");
        assert!(
            !code.contains("STRING"),
            "an unused handler is linked:\n{code}"
        );
        assert!(!code.contains("with_numbers"), "{code}");

        // With `fn-number`, the same function comes from the localized crate.
        let localized = Features::parse("fn-number");
        let code = write(&module(&functions, &localized, &custom, &locales, false));
        assert!(
            code.contains("(\"integer\", &__mf2::fn_number::INTEGER),"),
            "{code}"
        );
        // Still no unannotated hook: nothing in this corpus can receive one.
        assert!(!code.contains("with_numbers"), "{code}");
    }

    #[test]
    fn the_unannotated_hooks_need_a_placeholder_that_can_reach_them() {
        let locales = locales();
        let custom = BTreeMap::new();
        let both = Features::parse("fn-number,datetime");
        let code = write(&module(&[], &both, &custom, &locales, true));
        assert!(
            code.contains(".with_numbers(&__mf2::fn_number::NUMBERS)"),
            "{code}"
        );
        assert!(
            code.contains(".with_dates(&__mf2::fn_datetime::DATES)"),
            "{code}"
        );
    }

    #[test]
    fn a_corpus_without_dates_never_names_a_date_host() {
        let locales = locales();
        let custom = BTreeMap::new();
        let none = Features::default();
        let code = write(&module(&[], &none, &custom, &locales, false));
        assert!(code.contains("super::__mf2::__use_host!();"), "{code}");

        // `datetime` is not enough: nothing in this corpus can be a date.
        let dates = Features::parse("datetime,host-web-datetime-intl");
        let code = write(&module(&[], &dates, &custom, &locales, false));
        assert!(code.contains("super::__mf2::__use_host!();"), "{code}");

        // A date function, or a plain placeholder, which may be handed one.
        let time = ["time".to_owned()];
        let code = write(&module(&time, &dates, &custom, &locales, false));
        assert!(code.contains("super::__mf2::__use_host!(dates);"), "{code}");
        let code = write(&module(&[], &dates, &custom, &locales, true));
        assert!(code.contains("super::__mf2::__use_host!(dates);"), "{code}");
        // Without the feature, neither names one.
        let code = write(&module(&time, &none, &custom, &locales, true));
        assert!(code.contains("super::__mf2::__use_host!();"), "{code}");
    }

    #[test]
    fn only_intl_with_a_number_names_the_intl_number_host() {
        let locales = locales();
        let custom = BTreeMap::new();
        let integer = ["integer".to_owned()];
        // Without `number-intl`, a number names nothing more.
        let plain = Features::parse("fn-number");
        let code = write(&module(&integer, &plain, &custom, &locales, false));
        assert!(code.contains("super::__mf2::__use_host!();"), "{code}");
        // With it: a numeric function, or a plain placeholder.
        let intl = Features::parse("number-intl");
        let code = write(&module(&integer, &intl, &custom, &locales, false));
        assert!(
            code.contains("super::__mf2::__use_host!(numbers);"),
            "{code}"
        );
        let code = write(&module(&[], &intl, &custom, &locales, true));
        assert!(
            code.contains("super::__mf2::__use_host!(numbers);"),
            "{code}"
        );
        // No number in the corpus: no `Intl` number host.
        let code = write(&module(&[], &intl, &custom, &locales, false));
        assert!(code.contains("super::__mf2::__use_host!();"), "{code}");
        // Dates and numbers both.
        let both = Features::parse("number-intl,datetime,host-web-datetime-intl");
        let code = write(&module(&[], &both, &custom, &locales, true));
        assert!(
            code.contains("super::__mf2::__use_host!(dates numbers);"),
            "{code}"
        );
    }

    #[test]
    fn inline_mode_bakes_the_manifest_itself() {
        let locales = locales();
        let custom = BTreeMap::new();
        let features = Features::default();
        let mut m = module(&[], &features, &custom, &locales, false);
        // A manifest's first bytes: the magic, the version, the hash — the
        // escaping has to survive both text and non-text bytes.
        let bytes = b"MF2M\x00\x01\xf1^\xb0\xee\x12\xdc\xe0C\"\\";
        m.manifest_bytes = Some(bytes);
        let code = write(&m);
        assert!(
            code.contains(
                "$crate::__mf2::__tr_impl!(bytes b\"MF2M\\x00\\x01\\xf1^\\xb0\\xee\\x12\\xdc\\xe0C\\\"\\\\\" 0x43e0dc12eeb05ef1u64"
            ),
            "{code}"
        );
        assert!(!code.contains("/out/manifest.mf2m"), "{code}");
    }

    #[test]
    fn the_tr_wrapper_bakes_in_the_path_and_the_hash() {
        let locales = locales();
        let custom = BTreeMap::new();
        let features = Features::default();
        let code = write(&module(&[], &features, &custom, &locales, false));
        assert!(
            code.contains("$crate::__mf2::__tr_impl!(\"/out/manifest.mf2m\" 0x43e0dc12eeb05ef1u64 ; $crate ; $($t)*)"),
            "{code}"
        );
        assert!(
            code.contains("$crate::__mf2::__msg_id_impl!(\"/out/manifest.mf2m\" 0x43e0dc12eeb05ef1u64 ; $crate ; $($t)*)"),
            "{code}"
        );
    }

    #[test]
    fn locale_has_a_variant_per_tag() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let code = write(&module(&[], &features, &custom, &locales, false));
        for expected in [
            "pub enum Locale {\n    /// `en`\n    En,\n    /// `ar`\n    Ar,\n}",
            "pub const ALL: [Locale; 2] = [Locale::En, Locale::Ar];",
            "pub const SOURCE: Locale = Locale::En;",
            "Locale::Ar => \"ar\",",
            "Locale::Ar => __mf2::Dir::Rtl,\n            _ => __mf2::Dir::Ltr,",
            "impl ::core::str::FromStr for Locale {\n    type Err = __mf2::UnknownLocale;",
            "__mf2::__best_locale!(LANGUAGE_MATCHING, LOCALES, tag)",
            "impl ::core::fmt::Display for Locale {",
        ] {
            assert!(code.contains(expected), "{expected}:\n{code}");
        }
        assert!(
            block(&code, "__if_clap")
                .contains("impl __mf2::__generated::clap::ValueParserFactory for Locale {"),
            "{code}"
        );
        // A client-only application's setup alone carries the matching data.
        assert!(
            block(&code, "__if_leptos").contains(
                "        __mf2::__if_csr! {\n            let setup = setup.with_language_matching(&LANGUAGE_MATCHING);"
            ),
            "{code}"
        );
    }

    #[test]
    fn variants_and_constants_are_named_by_rule() {
        assert_eq!(variant("pt-BR"), "PtBr");
        assert_eq!(variant("es-419"), "Es419");
        assert_eq!(variant("zh-Hant-TW"), "ZhHantTw");
        assert_eq!(variant("und"), "Und");
        assert_eq!(constant("key"), "KEY");
        assert_eq!(constant("ns:warn-now.x"), "NS_WARN_NOW_X");
        assert_eq!(constant("caf\u{e9}"), "CAF_");
    }

    #[test]
    fn a_shared_variant_is_refused_and_a_shared_constant_left_out() {
        let features = Features::default();
        let custom = BTreeMap::new();
        let mut two = locales();
        two[0].tag = "pt-BR".to_owned();
        two[1].tag = "pt_br".to_owned();
        let m = module(&[], &features, &custom, &two, false);
        assert!(
            matches!(check(&m), Err(Error::LocaleVariant { variant, .. }) if variant == "PtBr")
        );

        // Every markup name MF2 allows is valid: one that gives no
        // constant, or the same one as another, is left out, not refused.
        let locales = locales();
        let mut m = module(&[], &features, &custom, &locales, false);
        let names = [
            "+:_\u{a1}".to_owned(),
            "a-b".to_owned(),
            "a.b".to_owned(),
            "ok".to_owned(),
        ];
        m.markup = &names;
        assert!(check(&m).is_ok());
        let code = write(&m);
        let ratatui = block(&code, "__if_ratatui");
        assert_eq!(ratatui.matches("pub const ").count(), 1, "{ratatui}");
        assert!(ratatui.contains("pub const OK:"), "{ratatui}");
    }

    #[test]
    fn markup_names_are_constants_with_their_hash() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let markup = ["key".to_owned(), "ns:warn".to_owned()];
        let mut m = module(&[], &features, &custom, &locales, false);
        m.markup = &markup;
        let code = write(&m);
        let ratatui = block(&code, "__if_ratatui");
        let key = super::hex_u64(mf2_catalog::markup_key("key"));
        assert!(
            ratatui.contains(&format!(
                "    pub const KEY: super::__mf2::ratatui::Markup =\n            super::__mf2::ratatui::Markup::new({key}, \"key\");"
            )),
            "{code}"
        );
        assert!(ratatui.contains("pub const NS_WARN:"), "{code}");
    }

    #[test]
    fn the_prelude_holds_what_the_build_has() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let mut m = module(&[], &features, &custom, &locales, false);
        let code = write(&m);
        let prelude = &code[code.find("pub mod prelude {").unwrap_or(0)..];
        for expected in [
            "pub use super::{Locale, msg_id, tr};",
            "pub use super::__mf2::{Tr, TrArgs, TrDyn, TrRich};",
            "super::__mf2::__if_mode! {\n        pub use super::{current_locale, set_locale};",
            "super::__mf2::__if_leptos! {\n        pub use super::preload_locale;",
            "super::__mf2::__if_native! {\n        pub use super::with_locale;",
        ] {
            assert!(prelude.contains(expected), "{expected}:\n{prelude}");
        }
        assert!(!prelude.contains("install") && !prelude.contains("markup"));

        m.emit = Emit::Module;
        let code = write(&m);
        let prelude = &code[code.find("pub mod prelude {").unwrap_or(0)..];
        assert!(
            prelude.contains(
                "super::__mf2::__if_leptos! {\n        pub use super::{current_locale, set_locale};"
            ),
            "{prelude}"
        );
        assert!(!prelude.contains("with_locale"), "{prelude}");
    }

    #[test]
    fn names_come_after_the_macro_they_expand() {
        let locales = locales();
        let features = Features::default();
        let custom = BTreeMap::new();
        let named = vec![true; locales.len()];
        let some = vec![false; locales.len()];
        let mut m = module(&[], &features, &custom, &locales, false);
        assert!(!write(&m).contains("pub fn name("));
        m.names = &some;
        assert!(!write(&m).contains("with_names"));
        m.names = &named;
        let code = write(&m);
        let name = code.find("pub fn name(self) -> __mf2::Tr").unwrap_or(0);
        assert!(
            name > code.find("macro_rules! __mf2_tr {").unwrap_or(usize::MAX),
            "{code}"
        );
        // A3's shape (3c): exported under hidden names, named by a `use`,
        // and no textual `tr` beside it, which would make the name ambiguous.
        for expected in [
            "#[doc(hidden)]\n#[macro_export]\nmacro_rules! __mf2_tr {",
            "pub use __mf2_tr as tr;",
            "#[doc(hidden)]\n#[macro_export]\nmacro_rules! __mf2_msg_id {",
            "pub use __mf2_msg_id as msg_id;",
        ] {
            assert!(code.contains(expected), "{expected}:\n{code}");
        }
        assert!(
            !code.contains("macro_rules! tr ") && !code.contains("macro_rules! msg_id "),
            "{code}"
        );
        assert!(
            code.contains("Locale::Ar => tr!(\"language.ar\"),"),
            "{code}"
        );
        // The switcher's names, by index, after the macro too.
        let at = code.find("fn __locale_name(").unwrap_or(0);
        assert!(at > code.find("macro_rules! __mf2_tr {").unwrap_or(usize::MAX));
        assert!(
            code.contains("let setup = setup.with_names(__locale_name);"),
            "{code}"
        );
    }
}
