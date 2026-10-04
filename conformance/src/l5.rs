//! Layer L5 — the suite through `tr!`.
//!
//! Where L4 compiles one message with `compile_str` and formats it, L5 is the
//! shipped road: a corpus `mf2-build` compiled, a manifest, a generated
//! module, and a call site the proc-macro checked and lowered to slots. The
//! four crates under `conformance/l5/` are that road — one per locale the
//! suite uses — and their build scripts derive everything from the vendored
//! suite, so no copy of it can drift.
//!
//! A test asserts one of two things:
//!
//! * the message is runtime-valid, so there is a call site: it must produce
//!   **the same record L4 produces** — the same string, the same errors, the
//!   same parts. A test whose `params` deliberately do not name the
//!   message's variables goes through the dynamic path and is marked `dyn`
//!   in the ledger;
//! * the spec refuses the message, so there is no call site: the **build**
//!   must have refused it, with exactly the error kinds the suite expects.
//!
//! Layer **L5d** is the same in the default configuration: the gated
//! functions are no longer a run-time *Unknown Function* as at L4d but a
//! **build rejection** — the build stops before a message can add formatting
//! code to the wasm — and everything else degrades exactly as L4d recorded.

use mf2::{Catalog, FormatContext, Formatter, Registry};
use mf2_l4_runner::{JsonParts, Record, record_from};
use mf2_runtime::BidiStrategy;

use crate::l4::DefaultOutcome;
use crate::ledger::DegradedKind;
use crate::matrix::TestKind;
use crate::suite::SuiteTest;

/// One locale's L5 crate, behind the two things this layer needs of it.
struct Crate {
    locale: &'static str,
    /// Its catalog, as its build wrote it.
    catalog: fn() -> &'static [u8],
    /// The manifest hash the catalog and the call sites agree on.
    manifest_hash: fn() -> u64,
    /// The closed-world registry its corpus needs.
    registry: fn() -> &'static Registry,
    /// The call site of an id, built afresh.
    case: fn(&str) -> Option<Case>,
    /// The MF2 error kinds the build refused an id with.
    rejected: fn(&str) -> Option<&'static [&'static str]>,
    /// What the build says about an id in the default configuration.
    gated: fn(&str) -> Option<&'static str>,
    /// Whether an id's call site passes its arguments by name.
    is_dyn: fn(&str) -> bool,
}

/// A call site's description, as the **render** layer takes it: the same
/// three shapes, with the locale crate forgotten (layer L6).
pub(crate) enum Description {
    Tr(mf2::Tr),
    Args(mf2::TrArgs),
    Dyn(mf2::TrDyn),
}

/// The three shapes a call site can have, erased across the four crates.
pub(crate) enum Case {
    EnUs(mf2_l5_en_us::Case),
    Und(mf2_l5_und::Case),
    Fr(mf2_l5_fr::Case),
    Ar(mf2_l5_ar::Case),
}

impl Case {
    /// The description itself, for a layer that renders rather than formats.
    pub(crate) fn description(&self) -> Description {
        macro_rules! lower {
            ($c:expr) => {
                match $c {
                    mf2_l5_en_us::Case::Tr(t) => Description::Tr(*t),
                    mf2_l5_en_us::Case::Args(t) => Description::Args(t.clone()),
                    mf2_l5_en_us::Case::Dyn(t) => Description::Dyn(t.clone()),
                }
            };
        }
        // The four crates' `Case` enums are the same shape — one file,
        // `include!`d — but they are four types, so this is four arms.
        match self {
            Case::EnUs(c) => lower!(c),
            Case::Und(c) => match c {
                mf2_l5_und::Case::Tr(t) => Description::Tr(*t),
                mf2_l5_und::Case::Args(t) => Description::Args(t.clone()),
                mf2_l5_und::Case::Dyn(t) => Description::Dyn(t.clone()),
            },
            Case::Fr(c) => match c {
                mf2_l5_fr::Case::Tr(t) => Description::Tr(*t),
                mf2_l5_fr::Case::Args(t) => Description::Args(t.clone()),
                mf2_l5_fr::Case::Dyn(t) => Description::Dyn(t.clone()),
            },
            Case::Ar(c) => match c {
                mf2_l5_ar::Case::Tr(t) => Description::Tr(*t),
                mf2_l5_ar::Case::Args(t) => Description::Args(t.clone()),
                mf2_l5_ar::Case::Dyn(t) => Description::Dyn(t.clone()),
            },
        }
    }

    fn write(&self, f: &Formatter<'_>, out: &mut dyn mf2::Sink, errs: &mut dyn mf2::ErrorSink) {
        match self {
            Case::EnUs(c) => c.write(f, out, errs),
            Case::Und(c) => c.write(f, out, errs),
            Case::Fr(c) => c.write(f, out, errs),
            Case::Ar(c) => c.write(f, out, errs),
        }
    }

    fn parts(&self, f: &Formatter<'_>, out: &mut dyn mf2::PartSink, errs: &mut dyn mf2::ErrorSink) {
        match self {
            Case::EnUs(c) => c.parts(f, out, errs),
            Case::Und(c) => c.parts(f, out, errs),
            Case::Fr(c) => c.parts(f, out, errs),
            Case::Ar(c) => c.parts(f, out, errs),
        }
    }
}

/// Every locale the suite uses. A test in any other locale has no crate, and
/// says so rather than passing quietly.
macro_rules! crates {
    ($($tag:literal => $krate:ident / $case:ident,)*) => {
        &[$(Crate {
            locale: $tag,
            catalog: $krate::catalog_bytes,
            manifest_hash: || $krate::MANIFEST_HASH,
            registry: $krate::registry,
            case: |id| $krate::case(id).map(Case::$case),
            rejected: $krate::rejected,
            gated: $krate::gated,
            is_dyn: $krate::is_dyn,
        },)*]
    };
}

static CRATES: &[Crate] = crates! {
    "en-US" => mf2_l5_en_us / EnUs,
    "und" => mf2_l5_und / Und,
    "fr" => mf2_l5_fr / Fr,
    "ar" => mf2_l5_ar / Ar,
};

/// The message id a test's corpus entry was given: its file and its position
/// in it, which is what `mf2-l5-gen` wrote and what `tr!` looked up.
#[must_use]
pub fn id(test: &SuiteTest) -> String {
    let file = test
        .key
        .file
        .strip_suffix(".json")
        .unwrap_or(&test.key.file)
        .replace('/', ".");
    format!("suite.{file}.t{:03}", test.index)
}

/// Whether `test`'s call site passes its arguments by name (the ledger's
/// `via = "dyn"`).
#[must_use]
pub fn is_dyn(test: &SuiteTest) -> bool {
    krate(test).is_some_and(|c| (c.is_dyn)(&id(test)))
}

fn krate(test: &SuiteTest) -> Option<&'static Crate> {
    CRATES.iter().find(|c| c.locale == test.locale)
}

/// Checks one test at L5: `Ok` = pass, `Err` = what failed.
pub fn check(test: &SuiteTest) -> Result<(), String> {
    let Some(krate) = krate(test) else {
        return Err(format!(
            "no L5 crate for locale {:?}; add one under conformance/l5/",
            test.locale
        ));
    };
    if test.kind != TestKind::Other {
        return check_rejected(test, krate);
    }
    let got = record(test, krate, (krate.registry)())?;
    crate::l4::judge(test, &got)
}

/// Checks one test at L5d, the default configuration.
///
/// A gated function is not a run-time error here but a **build** error: the
/// corpus does not compile with the feature off, which is what keeps a
/// translation from adding formatting code to the wasm. Everything else
/// degrades as at L4d, and the ledger holds the two columns to each other.
pub fn check_default(test: &SuiteTest) -> DefaultOutcome {
    let Some(krate) = krate(test) else {
        return DefaultOutcome::Fail(format!("no L5 crate for locale {:?}", test.locale));
    };
    if test.kind != TestKind::Other {
        return match check_rejected(test, krate) {
            Ok(()) => DefaultOutcome::Pass,
            Err(e) => DefaultOutcome::Fail(e),
        };
    }
    if let Some(detail) = (krate.gated)(&id(test)) {
        return DefaultOutcome::Degraded(DegradedKind::BuildReject, detail.to_owned());
    }
    let got = match record(test, krate, &mf2_l4_runner::DEFAULT_REGISTRY) {
        Ok(got) => got,
        Err(e) => return DefaultOutcome::Fail(e),
    };
    crate::l4::classify_default(test, &got)
}

/// The build refused the message, with exactly the kinds the suite expects.
fn check_rejected(test: &SuiteTest, krate: &Crate) -> Result<(), String> {
    let id = id(test);
    let Some(kinds) = (krate.rejected)(&id) else {
        return Err(format!(
            "the build accepted {id}, which the suite expects it to refuse ({:?})",
            test.exp_errors
        ));
    };
    let mut got: Vec<&str> = kinds.to_vec();
    got.sort_unstable();
    got.dedup();
    let mut want: Vec<&str> = test.exp_errors.iter().map(String::as_str).collect();
    want.sort_unstable();
    want.dedup();
    if got == want {
        Ok(())
    } else {
        Err(format!(
            "the build refused {id} with {got:?}, expected {want:?}"
        ))
    }
}

/// The call site of `test`, its catalog and the registry its corpus needs —
/// what a layer above L5 renders with.
pub(crate) fn inputs(test: &SuiteTest) -> Result<(Case, Catalog, &'static Registry), String> {
    let Some(krate) = krate(test) else {
        return Err(format!("no L5 crate for locale {:?}", test.locale));
    };
    let id = id(test);
    let Some(case) = (krate.case)(&id) else {
        return Err(format!(
            "no call site for {id}: the build did not carry it into the corpus"
        ));
    };
    let catalog = Catalog::new((krate.catalog)().to_vec(), (krate.manifest_hash)())
        .map_err(|e| format!("the L5 catalog of {} does not load: {e:?}", krate.locale))?;
    Ok((case, catalog, (krate.registry)()))
}

/// Formats the test's call site against its crate's catalog, as a record
/// comparable with L4's byte for byte.
fn record(test: &SuiteTest, krate: &Crate, registry: &'static Registry) -> Result<Record, String> {
    let id = id(test);
    let Some(case) = (krate.case)(&id) else {
        return Err(format!(
            "no call site for {id}: the build did not carry it into the corpus"
        ));
    };
    let catalog = Catalog::new((krate.catalog)().to_vec(), (krate.manifest_hash)())
        .map_err(|e| format!("the L5 catalog of {} does not load: {e:?}", krate.locale))?;
    let mut cx = FormatContext::new(&mf2::host_std::ZONES_HOST);
    cx.bidi = match test.bidi_isolation.as_deref() {
        Some("none") => BidiStrategy::None,
        _ => BidiStrategy::Default,
    };
    let f = Formatter::new(&catalog, registry, &cx);

    let mut text = String::new();
    let mut errors = Vec::new();
    case.write(&f, &mut text, &mut errors);

    let mut parts = JsonParts::default();
    let mut parts_errors = Vec::new();
    case.parts(&f, &mut parts, &mut parts_errors);

    record_from(text, &errors, &parts, &parts_errors)
}
