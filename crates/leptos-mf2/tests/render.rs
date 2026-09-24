//! Rendering a description through the view type (Phase 6 A2, A3).
//!
//! Server-side, because that is where a rendered description can be compared
//! as a string without a browser; the browser half is conformance L6 and the
//! `tools/e2e` checks.

use std::sync::Arc;

use leptos::prelude::*;
use leptos_mf2::{RequestI18n, Setup, Tr, TrArgs, install, markup, tr, tr_args1, tr_rich};
use mf2::{ArgValue, Catalog, Compiled, Dir, Function, MsgId, Registry, functions, markup_key};
use tachys::html::attribute::AttributeValue;
use tachys::view::RenderHtml;

static FUNCTIONS: [(&str, &dyn Function); 2] = [
    ("integer", &functions::INTEGER),
    ("string", &functions::STRING),
];
static REGISTRY: Registry = Registry::new(&FUNCTIONS);
static LOCALES: &[(&str, Dir)] = &[("en", Dir::Ltr), ("ar", Dir::Rtl)];

/// `install` is process-wide and idempotent, so every test calls this and the
/// first one wins. The manifest hash is only checked when a catalog is read
/// from bytes, which these tests do not do.
fn installed() {
    install(Setup::new(
        &REGISTRY,
        &mf2::host_std::HOST,
        0,
        "en",
        LOCALES,
    ));
}

fn compiled(source: &str, locale: &str) -> Compiled {
    mf2::compile_str(source, locale).expect("the message compiles")
}

fn catalog_of(source: &str, locale: &str) -> Arc<Catalog> {
    Arc::new(compiled(source, locale).catalog)
}

/// Runs `body` as a request would: inside an owner, with `catalog` provided.
fn in_request<R>(catalog: &Arc<Catalog>, body: impl FnOnce() -> R) -> R {
    installed();
    let owner = Owner::new();
    owner.with(|| {
        RequestI18n::new(Arc::clone(catalog)).provide();
        body()
    })
}

const ID: MsgId = Compiled::ID;

fn kbd_handlers() -> Box<[(u64, Arc<dyn leptos_mf2::MarkupHandler>)]> {
    [(
        markup_key("kbd"),
        markup(|children: AnyView| view! { <kbd>{children}</kbd> }),
    )]
    .into()
}

#[test]
fn a_simple_message_renders_its_text() {
    let catalog = catalog_of("Save", "en");
    let html = in_request(&catalog, || RenderHtml::to_html(tr(ID)));
    assert_eq!(html, "Save");
}

#[test]
fn text_is_escaped_the_way_tachys_escapes_it() {
    let catalog = catalog_of("a < b & c", "en");
    let html = in_request(&catalog, || RenderHtml::to_html(tr(ID)));
    assert_eq!(html, "a &lt; b &amp; c");
}

#[test]
fn a_pattern_formats_its_arguments() {
    let catalog = catalog_of("You have {$n :integer} messages", "en");
    let html = in_request(&catalog, || {
        RenderHtml::to_html(tr_args1(ID, ArgValue::from(3)))
    });
    // A number is LTR in an LTR message, so the Default Bidi Strategy adds
    // nothing; the string case below is where the isolates appear.
    assert_eq!(html, "You have 3 messages");
}

#[test]
fn a_string_is_plain_and_a_text_child_is_isolated() {
    let catalog = catalog_of("Hello, {$name}!", "en");
    let (html, string) = in_request(&catalog, || {
        let d: TrArgs = tr_args1(ID, ArgValue::str_static("Ada"));
        (RenderHtml::to_html(d.clone()), d.to_string())
    });
    assert_eq!(html, "Hello, \u{2068}Ada\u{2069}!");
    // A `String` is what a program consumes, so no invisible controls in it
    // (04 §9).
    assert_eq!(string, "Hello, Ada!");
    // …and the displayed form is there when the String goes back into the
    // page.
    let displayed = in_request(&catalog, || {
        tr_args1(ID, ArgValue::str_static("Ada")).to_display_string()
    });
    assert_eq!(displayed, "Hello, \u{2068}Ada\u{2069}!");
}

#[test]
fn a_description_is_an_attribute_value() {
    let catalog = catalog_of("Search \"everything\"", "en");
    let mut buf = String::new();
    in_request(&catalog, || {
        AttributeValue::to_html(tr(ID), "placeholder", &mut buf);
    });
    assert_eq!(buf, " placeholder=\"Search &quot;everything&quot;\"");
}

/// An attribute is isolated or plain by its name (04 §9, owner,
/// 2026-09-24): `value=` is submitted with a form and `data-*` is read by a
/// script, so the marks would be junk there; `title=` is read by a person.
#[test]
fn an_attribute_is_isolated_or_plain_by_its_name() {
    let catalog = catalog_of("Hello, {$name}!", "en");
    let attr = |key: &str| {
        let mut buf = String::new();
        in_request(&catalog, || {
            let d: TrArgs = tr_args1(ID, ArgValue::str_static("Ada"));
            AttributeValue::to_html(d, key, &mut buf);
        });
        buf
    };
    for key in ["value", "data-x", "href", "download", "VALUE", "Data-X"] {
        assert_eq!(
            attr(key),
            format!(" {key}=\"Hello, Ada!\""),
            "{key}= is plain"
        );
    }
    for key in [
        "title",
        "aria-label",
        "placeholder",
        "alt",
        "database",
        "values",
    ] {
        assert_eq!(
            attr(key),
            format!(" {key}=\"Hello, \u{2068}Ada\u{2069}!\""),
            "{key}= is isolated"
        );
    }
}

#[test]
fn a_missing_catalog_renders_empty_text_and_does_not_panic() {
    installed();
    // No context and no catalog for the default locale: the one state in
    // which nothing can be formatted (04 §5).
    let html = Owner::new().with(|| RenderHtml::to_html(tr(MsgId::from_raw(7))));
    // tachys writes a single space for empty escaped text, which is what
    // `<&str as RenderHtml>` does and what delegating to it gives us.
    assert_eq!(html, " ");
}

#[test]
fn markup_becomes_elements_in_the_message_s_own_order() {
    let catalog = catalog_of("Press {#kbd}Esc{/kbd} to close", "en");
    let html = in_request(&catalog, || {
        RenderHtml::to_html(tr_rich(TrArgs::from(tr(ID)), kbd_handlers()))
    });
    // The trailing `<!>` is the placeholder tachys keeps after any `Vec`
    // view; it is a comment node, so it contributes no text.
    assert_eq!(html, "Press <kbd>Esc<!></kbd> to close<!>");
}

#[test]
fn an_open_that_is_never_closed_closes_at_the_end_of_the_pattern() {
    // MF2 does not require pairing, and the suite tests lone opens; the
    // renderer closes them rather than dropping the rest (04 §7).
    let catalog = catalog_of("Press {#kbd}Esc to close", "en");
    let html = in_request(&catalog, || {
        RenderHtml::to_html(tr_rich(TrArgs::from(tr(ID)), kbd_handlers()))
    });
    assert!(html.contains("<kbd"), "{html}");
    assert!(html.contains("Esc to close"), "{html}");
}

#[test]
fn a_close_with_no_open_is_dropped() {
    let catalog = catalog_of("Press Esc{/kbd} to close", "en");
    let html = in_request(&catalog, || {
        RenderHtml::to_html(tr_rich(TrArgs::from(tr(ID)), kbd_handlers()))
    });
    assert!(!html.contains("<kbd"), "{html}");
    assert!(html.contains("Press Esc"), "{html}");
}

#[test]
fn a_rich_message_with_no_handler_renders_only_its_text() {
    // What L5 asserts: markup writes no text, and handlers are all or none.
    let catalog = catalog_of("Press {#kbd}Esc{/kbd} to close", "en");
    let html = in_request(&catalog, || {
        RenderHtml::to_html(tr_rich(TrArgs::from(tr(ID)), Vec::new().into()))
    });
    assert_eq!(html, "Press Esc to close<!>");
}

#[test]
fn a_text_prop_captures_the_request_s_catalog() {
    // D9: leptos_meta reads `<Title text>` after rendering, outside the
    // request owner, so the conversion captures inside it (04 §5).
    let catalog = catalog_of("Inbox", "en");
    let prop: TextProp = in_request(&catalog, || TextProp::from(tr(ID)));
    // Read outside any owner, as leptos_meta does.
    assert_eq!(prop.get().as_str(), "Inbox");
}

#[test]
fn the_description_of_a_call_site_is_still_four_bytes() {
    assert_eq!(size_of::<Tr>(), 4);
}
