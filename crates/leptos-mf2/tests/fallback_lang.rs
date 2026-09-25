//! `mark-fallback-lang` on the server (Phase 7 A14): a message the catalog
//! borrowed from another locale renders inside `<span lang>`, and nothing
//! else changes (`plans/15-phase-7-work-order.md` §"A14 — design").
//!
//! The negative control is this file with the feature off: it fails every
//! assertion that expects a span. The browser half — hydration adopting the
//! span, a switch adding and removing it — is `tools/e2e/checks/demo.mjs`.

// The Leptos line under test (`plans/04-leptos-integration.md` §10).
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate leptos_0_8 as leptos;
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos-0-9")))]
extern crate tachys_0_2 as tachys;

use std::sync::Arc;

use leptos::prelude::*;
use leptos_mf2::{RequestI18n, Setup, TrArgs, install, markup, tr, tr_rich};
use mf2::{Catalog, Dir, Function, MsgId, Registry, functions, markup_key};
use mf2_catalog::writer::{self, Options};
use tachys::html::attribute::AttributeValue;
use tachys::view::RenderHtml;

static FUNCTIONS: [(&str, &dyn Function); 1] = [("string", &functions::STRING)];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static LOCALES: &[(&str, Dir)] = &[("en", Dir::Ltr), ("fr", Dir::Ltr), ("ar", Dir::Rtl)];

const ID: MsgId = MsgId::from_raw(0);

/// A one-message catalog for `locale` whose text is `source`, recorded as
/// borrowed from `lender` when there is one — what `mf2-build` writes for a
/// message a locale lacks under `missing = "fallback"`.
fn catalog(source: &str, locale: &str, dir: Dir, lender: Option<&str>) -> Arc<Catalog> {
    let parsed = mf2_syntax::parse_model(source);
    let model = parsed.message.expect("the message parses");
    let analysis = mf2_syntax::analyze(&model);
    let slots: Vec<&str> = analysis.externals.iter().map(|n| &*n.nfc).collect();
    let mut options = Options::new(locale, dir);
    if let Some(lender) = lender {
        options.fallback.push((0, lender.to_owned()));
    }
    let (bytes, manifest) = writer::single(&model, &slots, &options).expect("the catalog writes");
    Arc::new(Catalog::new(bytes, manifest.hash()).expect("the catalog reads"))
}

fn in_request<R>(catalog: &Arc<Catalog>, body: impl FnOnce() -> R) -> R {
    install(Setup::new(
        &REGISTRY,
        &mf2::host_std::HOST,
        0,
        "en",
        LOCALES,
    ));
    let owner = Owner::new();
    owner.with(|| {
        RequestI18n::new(Arc::clone(catalog)).provide();
        body()
    })
}

fn kbd_handlers() -> Box<[(u64, Arc<dyn leptos_mf2::MarkupHandler>)]> {
    [(
        markup_key("kbd"),
        markup(|children: AnyView| view! { <kbd>{children}</kbd> }),
    )]
    .into()
}

#[test]
fn a_borrowed_message_renders_inside_a_span_with_the_lender_s_lang() {
    let fr = catalog("Save", "fr", Dir::Ltr, Some("en"));
    let html = in_request(&fr, || RenderHtml::to_html(tr(ID)));
    // Same direction as the page: no `dir`.
    assert_eq!(html, "<span lang=\"en\">Save</span>");
}

#[test]
fn a_lender_of_the_other_direction_adds_dir() {
    let ar = catalog("Save", "ar", Dir::Rtl, Some("en"));
    let html = in_request(&ar, || RenderHtml::to_html(tr(ID)));
    assert_eq!(html, "<span lang=\"en\" dir=\"ltr\">Save</span>");
}

#[test]
fn an_own_message_has_no_span() {
    let fr = catalog("Enregistrer", "fr", Dir::Ltr, None);
    let html = in_request(&fr, || RenderHtml::to_html(tr(ID)));
    assert_eq!(html, "Enregistrer");
}

#[test]
fn a_borrowed_rich_message_is_wrapped_whole() {
    let source = "Press {#kbd}Esc{/kbd} to close";
    let rich = || tr_rich(TrArgs::from(tr(ID)), kbd_handlers());
    let own = in_request(&catalog(source, "fr", Dir::Ltr, None), || {
        RenderHtml::to_html(rich())
    });
    let borrowed = in_request(&catalog(source, "fr", Dir::Ltr, Some("en")), || {
        RenderHtml::to_html(rich())
    });
    assert_eq!(own, "Press <kbd>Esc<!></kbd> to close<!>");
    assert_eq!(
        borrowed,
        "<span lang=\"en\">Press <kbd>Esc<!></kbd> to close<!></span><!>"
    );
}

/// What follows a wrapped message writes, and so hydrates, exactly as it
/// would after an unwrapped one: the `<!>` separators are where tachys puts
/// them around a text.
#[test]
fn text_around_a_wrapped_message_separates_as_around_an_unwrapped_one() {
    let around = || RenderHtml::to_html(view! { <p>{"a"} {tr(ID)} {"b"}</p> });
    let own = in_request(&catalog("Save", "fr", Dir::Ltr, None), around);
    let borrowed = in_request(&catalog("Save", "fr", Dir::Ltr, Some("en")), around);
    assert_eq!(own, "<p>a<!>Save<!>b</p>");
    assert_eq!(
        borrowed,
        own.replace("Save", "<span lang=\"en\">Save</span>")
    );
}

/// An empty borrowed text writes the single space tachys writes for any
/// empty text, inside the span, so hydration has a text node to adopt.
#[test]
fn an_empty_borrowed_message_keeps_its_text_node() {
    let fr = catalog("", "fr", Dir::Ltr, Some("en"));
    let html = in_request(&fr, || RenderHtml::to_html(tr(ID)));
    assert_eq!(html, "<span lang=\"en\"> </span>");
}

/// HTML can mark an attribute's language only through its element, and a
/// `String` carries no markup: both stay as they were (documented in
/// `docs/accessibility.md`).
#[test]
fn an_attribute_and_a_string_are_unmarked() {
    let fr = catalog("Save", "fr", Dir::Ltr, Some("en"));
    let (attr, string) = in_request(&fr, || {
        let mut buf = String::new();
        AttributeValue::to_html(tr(ID), "title", &mut buf);
        (buf, tr(ID).to_string())
    });
    assert_eq!(attr, " title=\"Save\"");
    assert_eq!(string, "Save");
}
