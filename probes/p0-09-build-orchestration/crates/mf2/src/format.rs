//! Server-side catalogs and a probe-sized formatter (patterns, markup
//! stripped to its content, `.match` with exact → plural category → `*`).

use std::sync::OnceLock;

use p09_catalog::{Body, Catalog, Func, Key, Part};

use crate::error::InstallError;
use crate::{ArgValue, CatalogFile, MsgId};

struct Installed {
    files: &'static [CatalogFile],
    catalogs: Vec<Catalog>,
    current: usize,
}

static INSTALLED: OnceLock<Installed> = OnceLock::new();

/// Installs the embedded catalogs (server start-up). The current locale is
/// `P09_LOCALE` if set and present, else `default`.
pub fn install(files: &'static [CatalogFile], manifest_hash: u64, default: &str) {
    if let Err(e) = try_install(files, manifest_hash, default) {
        leptos::logging::error!("mf2: {e}");
    }
}

fn try_install(
    files: &'static [CatalogFile],
    manifest_hash: u64,
    default: &str,
) -> Result<(), InstallError> {
    let mut catalogs = Vec::with_capacity(files.len());
    for f in files {
        let c = Catalog::decode(f.bytes)?;
        if c.manifest_hash != manifest_hash {
            return Err(InstallError::Skew {
                locale: c.locale,
                found: c.manifest_hash,
                expected: manifest_hash,
            });
        }
        catalogs.push(c);
    }
    let want = std::env::var("P09_LOCALE").unwrap_or_else(|_| default.to_owned());
    let current = catalogs
        .iter()
        .position(|c| c.locale == want)
        .or_else(|| catalogs.iter().position(|c| c.locale == default))
        .unwrap_or(0);
    INSTALLED
        .set(Installed {
            files,
            catalogs,
            current,
        })
        .map_err(|_| InstallError::Twice)
}

/// Bytes of an embedded catalog by file name (for `/i18n/<file>`).
pub fn catalog_bytes(file: &str) -> Option<&'static [u8]> {
    INSTALLED
        .get()?
        .files
        .iter()
        .find(|f| f.file == file)
        .map(|f| f.bytes)
}

/// The current locale tag, if catalogs are installed.
pub fn current_locale() -> Option<&'static str> {
    let i = INSTALLED.get()?;
    Some(i.catalogs.get(i.current)?.locale.as_str())
}

fn category(locale: &str, n: i64) -> &'static str {
    let lang = locale.split('-').next().unwrap_or(locale);
    let n = n.unsigned_abs();
    match lang {
        "pl" => {
            if n == 1 {
                "one"
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                "few"
            } else {
                "many"
            }
        }
        _ => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

fn render(parts: &[Part], args: &[ArgValue], out: &mut String) {
    for p in parts {
        match p {
            Part::Text(t) | Part::Lit(t) => out.push_str(t),
            Part::Var(slot) => match args.get(*slot as usize) {
                Some(a) => out.push_str(&a.text()),
                None => out.push_str("{$?}"),
            },
            Part::MarkupOpen(_) | Part::MarkupClose(_) | Part::MarkupStandalone(_) => {}
        }
    }
}

/// Formats a message in the current locale; empty if no catalog is installed
/// (always the case on the client in this probe).
pub fn format(id: MsgId, args: &[ArgValue]) -> String {
    let mut out = String::new();
    let Some(inst) = INSTALLED.get() else {
        return out;
    };
    let Some(cat) = inst.catalogs.get(inst.current) else {
        return out;
    };
    let Some(Some(msg)) = cat.messages.get(id.0 as usize) else {
        return out;
    };
    match &msg.body {
        Body::Pattern(p) => render(p, args, &mut out),
        Body::Select {
            selectors,
            variants,
        } => {
            let mut best: Option<(Vec<u8>, usize)> = None;
            'v: for (vi, v) in variants.iter().enumerate() {
                let mut score = Vec::with_capacity(selectors.len());
                for (s, k) in selectors.iter().zip(&v.keys) {
                    let arg = args.get(s.slot as usize);
                    let rank = match k {
                        Key::Star => 2,
                        Key::Lit(k) => {
                            let exact = arg.is_some_and(|a| a.text() == k.as_str());
                            let plural = matches!(s.func, Func::Integer | Func::Number)
                                && arg
                                    .and_then(ArgValue::int)
                                    .is_some_and(|n| category(&cat.locale, n) == k);
                            if exact {
                                0
                            } else if plural {
                                1
                            } else {
                                continue 'v;
                            }
                        }
                    };
                    score.push(rank);
                }
                if best.as_ref().is_none_or(|(b, _)| score < *b) {
                    best = Some((score, vi));
                }
            }
            if let Some((_, vi)) = best
                && let Some(v) = variants.get(vi)
            {
                render(&v.pattern, args, &mut out);
            }
        }
    }
    out
}
