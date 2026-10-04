// What every conformance L5 crate is, beyond its generated module and its
// generated call sites.
//
// One file, `include!`d by each of the four locale crates, because they
// differ in exactly one thing: which locale of the suite they carry. It is
// included rather than a crate of its own because `Case` has to be built by
// *this* crate's `tr!`, which is this crate's generated module's.

/// One entry of [`CASES`]: an id, and the call site that formats it.
pub type Site = (&'static str, fn() -> Case);

/// A call site of the suite, as `cases.rs` built it — the three shapes `tr!`
/// can expand to, plus the dynamic path the deliberate mismatches take.
pub enum Case {
    /// A message with no arguments.
    Tr(::mf2::Tr),
    /// A message whose arguments are exactly its variables.
    Args(::mf2::TrArgs),
    /// A message whose `params` deliberately do not name its variables, so
    /// they are matched by name at run time (the ledger's `via = "dyn"`).
    Dyn(::mf2::TrDyn),
}

impl Case {
    /// Formats it into `out`.
    pub fn write(
        &self,
        f: &::mf2::Formatter<'_>,
        out: &mut dyn ::mf2::Sink,
        errs: &mut dyn ::mf2::ErrorSink,
    ) {
        match self {
            Case::Tr(t) => t.write(f, out, errs),
            Case::Args(t) => t.write(f, out, errs),
            Case::Dyn(t) => t.write(f, out, errs),
        }
    }

    /// Formats it to parts.
    pub fn parts(
        &self,
        f: &::mf2::Formatter<'_>,
        out: &mut dyn ::mf2::PartSink,
        errs: &mut dyn ::mf2::ErrorSink,
    ) {
        match self {
            Case::Tr(t) => t.parts(f, out, errs),
            Case::Args(t) => t.parts(f, out, errs),
            Case::Dyn(t) => t.parts(f, out, errs),
        }
    }
}

/// The call site of `id`, built afresh.
#[must_use]
pub fn case(id: &str) -> Option<Case> {
    CASES.iter().find(|(name, _)| *name == id).map(|(_, f)| f())
}

/// The MF2 error kinds the build refused `id` with, if it refused it.
#[must_use]
pub fn rejected(id: &str) -> Option<&'static [&'static str]> {
    REJECTED
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, kinds)| *kinds)
}

/// What the build says about `id` in the default configuration (L5d): the
/// error that refuses the corpus because a function's client feature is off.
#[must_use]
pub fn gated(id: &str) -> Option<&'static str> {
    GATED
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, detail)| *detail)
}

/// Whether `id`'s call site passes its arguments by name.
#[must_use]
pub fn is_dyn(id: &str) -> bool {
    VIA_DYN.contains(&id)
}

/// This crate's catalog, as the build wrote it.
///
/// Server-side: the generated `catalog` table is `ssr`-only, because a
/// client fetches its one locale rather than carrying them all (B6). A crate
/// that includes this file and builds for the browser — conformance L6 in
/// the browser does — gets its catalog over the wire like any client.
#[cfg(feature = "ssr")]
#[must_use]
pub fn catalog_bytes() -> &'static [u8] {
    catalog(SOURCE_LOCALE).unwrap_or(&[])
}

/// A typed `datetime` parameter, parsed as a date/time literal — the same
/// conversion L4's runner makes (`mf2_l4_runner::ArgSpec::date_time`).
///
/// Only with a date formatter (any of the four, as L7's `set-build` counts
/// them): conformance L7's default-configuration pages include this file
/// without one, and hold no message that takes a date.
#[cfg(any(
    feature = "host-std-datetime-icu",
    feature = "host-web-datetime-icu",
    feature = "host-std-datetime-iso",
    feature = "host-web-datetime-intl"
))]
#[must_use]
pub fn date_time(literal: &str) -> ::mf2::ArgValue {
    match ::mf2::fn_datetime::parse_literal(literal) {
        Some(value) => ::mf2::ArgValue::from(value),
        None => opaque(),
    }
}

/// A value no function takes — the suite's booleans and objects, as L4's
/// runner passes them.
#[must_use]
pub fn opaque() -> ::mf2::ArgValue {
    struct Opaque;

    impl ::mf2::CustomValue for Opaque {}

    ::mf2::ArgValue::custom(Opaque)
}
