//! Layer L6 — the suite through the **view type**
//! (`plans/01-conformance.md` §3; `plans/14-phase-6-work-order.md` A6).
//!
//! L5 formats a call site with a formatter the harness built. L6 renders the
//! same call site the way a page does: `<Tr as RenderHtml>::to_html`, with
//! the catalog coming from the request context, the registry from the same
//! place, and the errors discarded — which is the release client's policy
//! (`plans/03-runtime.md` §8) and therefore what a user actually sees.
//!
//! Two things are asserted, and they are different questions:
//!
//! * **text** — what tachys wrote must be `exp`, escaped as tachys escapes a
//!   text node. This is the whole suite, and it is where a rendering bug
//!   shows up as a wrong page rather than a wrong `String`;
//! * **markup** — a message with markup is rendered again through
//!   [`Flat`](leptos_mf2::Flat) handlers, one marker element per markup
//!   part, and the markers' order, kind, name and options must equal the
//!   markup entries of `expParts`. That is the only place the *renderer's*
//!   view of markup can be checked against the suite, because a `String` has
//!   no markup in it at all.
//!
//! Layer **L6d** is the same in the default configuration, and differs from
//! L6 exactly as L5d differs from L5: a gated function is a **build**
//! rejection, not a run-time one.
//!
//! What L6 does *not* re-assert is the error list: a description formatted
//! for the page discards errors by design, and L4 and L5 already hold the
//! suite to them.

use std::sync::{Arc, OnceLock};

use leptos::prelude::*;
use leptos_mf2::{Flat, RequestI18n, Setup, TrArgs, markup};
use mf2::{Dir, MarkupKind, MarkupPart, Registry, markup_key};
use mf2_runtime::BidiStrategy;
use serde_json::Value;
use tachys::view::RenderHtml;

use crate::l4::DefaultOutcome;
use crate::l5::{self, Description};
use crate::suite::SuiteTest;

/// The element a [`Flat`] handler renders for one markup part. A custom
/// element name, so that nothing in the suite's own text can be mistaken for
/// one.
const MARKER: &str = "mf2-mark";

/// The locale table the render layer is installed with. Only `dir_of` and
/// `locales()` read it, and L6 provides its catalog per test, so the four
/// locales the suite uses are all it needs.
static LOCALES: &[(&str, Dir)] = &[
    ("en-US", Dir::Ltr),
    ("und", Dir::Ltr),
    ("fr", Dir::Ltr),
    ("ar", Dir::Rtl),
];

/// Installs the render layer once per process.
///
/// The registry installed here is never the one used: every test provides
/// its own through [`RequestI18n::with_registry`], because the four L5
/// crates have four closed worlds and L6d needs the default one as well.
/// The manifest hash is likewise unused — it gates `read`, and L6 builds its
/// catalogs directly, as L5 does.
fn installed() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        leptos_mf2::install(Setup::new(
            &mf2_l4_runner::DEFAULT_REGISTRY,
            &mf2::host_std::HOST,
            0,
            "en-US",
            LOCALES,
        ));
    });
}

/// Checks one test at L6.
pub fn check(test: &SuiteTest) -> Result<(), String> {
    let (case, catalog, registry) = l5::inputs(test)?;
    render_and_judge(test, &case.description(), catalog, registry)
}

/// Checks one test at L6d, the default configuration.
///
/// The degradations are **L5d's**, and deliberately not restated here: with
/// the default features a gated function is a build rejection, a locale-only
/// option value is an unsupported operation, and numbers come out with
/// neutral digits — none of which is about rendering. What L6d adds is that
/// a test L5d passes must also *render* correctly with the default
/// registry.
pub fn check_default(test: &SuiteTest) -> DefaultOutcome {
    match l5::check_default(test) {
        DefaultOutcome::Pass => {}
        other => return other,
    }
    let (case, catalog, _) = match l5::inputs(test) {
        Ok(inputs) => inputs,
        Err(e) => return DefaultOutcome::Fail(e),
    };
    let rendered = render(
        test,
        &case.description(),
        catalog,
        &mf2_l4_runner::DEFAULT_REGISTRY,
    );
    match judge_text(test, &rendered.text) {
        Ok(()) => DefaultOutcome::Pass,
        Err(e) => DefaultOutcome::Fail(e),
    }
}

/// What one render produced.
struct Rendered {
    /// The text a user sees, unescaped back from the HTML tachys wrote.
    text: String,
    /// One record per markup part, in render order.
    markup: Vec<Marker>,
}

/// A markup part, as the marker element recorded it.
#[derive(PartialEq, Eq, Debug)]
struct Marker {
    kind: String,
    name: String,
    options: Vec<(String, String)>,
}

fn render_and_judge(
    test: &SuiteTest,
    description: &Description,
    catalog: mf2::Catalog,
    registry: &'static Registry,
) -> Result<(), String> {
    let rendered = render(test, description, catalog, registry);
    judge_text(test, &rendered.text)?;
    judge_markup(test, &rendered.markup)
}

/// Renders `description` as a page would, inside a request owner.
fn render(
    test: &SuiteTest,
    description: &Description,
    catalog: mf2::Catalog,
    registry: &'static Registry,
) -> Rendered {
    installed();
    let catalog = Arc::new(catalog);
    let bidi = match test.bidi_isolation.as_deref() {
        Some("none") => BidiStrategy::None,
        _ => BidiStrategy::Default,
    };
    let names = markup_names(test);
    let owner = Owner::new();
    owner.with(|| {
        RequestI18n::new(Arc::clone(&catalog))
            .with_registry(registry)
            .with_bidi(bidi)
            .provide();
        let text = unescape(&match description {
            Description::Tr(t) => RenderHtml::to_html(*t),
            Description::Args(t) => RenderHtml::to_html(t.clone()),
            Description::Dyn(t) => RenderHtml::to_html(t.clone()),
        });
        let markup = if names.is_empty() {
            Vec::new()
        } else {
            markers(&rich_html(description, &names))
        };
        Rendered { text, markup }
    })
}

/// The same description with a flat recorder handler per markup name, as
/// HTML.
///
/// `TrDyn` has no rich form — its arguments are matched by name at run time,
/// which is the one shape `tr_rich` does not wrap — so a dynamic test with
/// markup records nothing and says so through an empty list.
fn rich_html(description: &Description, names: &[String]) -> String {
    let args: TrArgs = match description {
        Description::Tr(t) => TrArgs::from(*t),
        Description::Args(t) => t.clone(),
        Description::Dyn(_) => return String::new(),
    };
    let handlers: Vec<_> = names
        .iter()
        .map(|name| (markup_key(name), markup(Flat(recorder))))
        .collect();
    RenderHtml::to_html(mf2::tr_rich(args, handlers.into()))
}

/// One marker element per markup part: the kind, the name, and every option
/// the catalog resolved, as attributes.
fn recorder(part: &MarkupPart<'_>) -> AnyView {
    let kind = match part.kind() {
        MarkupKind::Open => "open",
        MarkupKind::Standalone => "standalone",
        MarkupKind::Close => "close",
    };
    let mut options = String::new();
    for (name, value) in part.options() {
        options.push_str(name);
        options.push('=');
        options.push_str(&mf2_l4_runner::value_text(value));
        options.push(';');
    }
    view! {
        <mf2-mark data-kind=kind data-name=part.name().to_owned() data-options=options></mf2-mark>
    }
    .into_any()
}

/// Whether `test` expects markup parts, so that a run can show the markup
/// half of this layer is not vacuous.
#[must_use]
pub fn expects_markup(test: &SuiteTest) -> bool {
    !expected_markup(test).is_empty()
}

/// The markup names `expParts` expects, in first-seen order and without
/// repeats — which is what a call site supplies handlers for.
fn markup_names(test: &SuiteTest) -> Vec<String> {
    let Some(Value::Array(parts)) = &test.exp_parts else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for part in parts {
        if part.get("type").and_then(Value::as_str) == Some("markup")
            && let Some(name) = part.get("name").and_then(Value::as_str)
            && !names.iter().any(|n| n == name)
        {
            names.push(name.to_owned());
        }
    }
    names
}

/// What tachys wrote must be `exp`, escaped the way it escapes a text node.
fn judge_text(test: &SuiteTest, got: &str) -> Result<(), String> {
    let Some(exp) = &test.exp else {
        // Nothing to compare: the test's assertion is about errors, which a
        // rendered description discards by design. Rendering it at all —
        // without a panic — is the assertion here.
        return Ok(());
    };
    if got == exp {
        Ok(())
    } else {
        Err(format!("rendered {got:?}, expected {exp:?}"))
    }
}

/// The markers must be `expParts`' markup entries: same order, same kind,
/// same name, same options.
fn judge_markup(test: &SuiteTest, got: &[Marker]) -> Result<(), String> {
    let want = expected_markup(test);
    if want.is_empty() {
        return Ok(());
    }
    if got.is_empty() {
        // A dynamic call site has no rich form; the suite has none of these
        // today, and if one appears it should say so rather than pass.
        return Err(format!(
            "expected {} markup part(s), and the call site has no rich form",
            want.len()
        ));
    }
    if got == want.as_slice() {
        Ok(())
    } else {
        Err(format!("markup rendered {got:?}, expected {want:?}"))
    }
}

/// The markup entries of `expParts`, in order.
fn expected_markup(test: &SuiteTest) -> Vec<Marker> {
    let Some(Value::Array(parts)) = &test.exp_parts else {
        return Vec::new();
    };
    parts
        .iter()
        .filter(|part| part.get("type").and_then(Value::as_str) == Some("markup"))
        .map(|part| Marker {
            kind: part
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            name: part
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            options: match part.get("options") {
                Some(Value::Object(map)) => {
                    map.iter().map(|(k, v)| (k.clone(), json_text(v))).collect()
                }
                _ => Vec::new(),
            },
        })
        .collect()
}

/// A suite option value as text, the way `expParts` writes it.
fn json_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The marker elements of a rendered fragment, in document order.
///
/// A hand-written scan rather than a parser: the input is HTML *we* wrote,
/// the element is ours, and a dependency for this would be a dependency in
/// the conformance harness forever.
fn markers(html: &str) -> Vec<Marker> {
    let open = format!("<{MARKER} ");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find(&open) {
        let after = &rest[at + open.len()..];
        let Some(end) = after.find('>') else { break };
        let tag = &after[..end];
        out.push(Marker {
            kind: attr(tag, "data-kind"),
            name: attr(tag, "data-name"),
            options: parse_options(&attr(tag, "data-options")),
        });
        rest = &after[end..];
    }
    out
}

/// One attribute of a start tag we wrote, unescaped.
fn attr(tag: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    let Some(at) = tag.find(&key) else {
        return String::new();
    };
    let after = &tag[at + key.len()..];
    let end = after.find('"').unwrap_or(after.len());
    unescape_attr(&after[..end])
}

/// `a=1;b=2;` back into pairs.
fn parse_options(text: &str) -> Vec<(String, String)> {
    text.split(';')
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            entry
                .split_once('=')
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
        })
        .collect()
}

/// The inverse of the escaping tachys applies to a text node, which is
/// `&`, `<` and `>` and nothing else.
fn unescape(html: &str) -> String {
    // A `simple` message that is empty renders as one space, because that is
    // what `<&str as RenderHtml>` writes for empty escaped text.
    if html == " " {
        return String::new();
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let (entity, len) = if tail.starts_with("&amp;") {
            ('&', 5)
        } else if tail.starts_with("&lt;") {
            ('<', 4)
        } else if tail.starts_with("&gt;") {
            ('>', 4)
        } else {
            ('&', 1)
        };
        out.push(entity);
        rest = &tail[len..];
    }
    out.push_str(rest);
    out
}

/// An attribute value we wrote, unescaped: tachys escapes `&` and `"` there.
fn unescape_attr(value: &str) -> String {
    unescape(value).replace("&quot;", "\"")
}
