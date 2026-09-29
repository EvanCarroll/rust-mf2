//! Rendering a description through the view type (Phase 6 A2, A3).
//!
//! Server-side, because that is where a rendered description can be compared
//! as a string without a browser; the browser half is conformance L6 and the
//! `tools/e2e` checks.

// The Leptos line under test: `mf2` names each line under a name of its
// own, and a test binds the one that is on.
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos")))]
extern crate leptos_0_8 as leptos;
#[cfg(feature = "leptos")]
extern crate leptos_0_9 as leptos;
#[cfg(all(feature = "leptos-0-8", not(feature = "leptos")))]
extern crate tachys_0_2 as tachys;
#[cfg(feature = "leptos")]
extern crate tachys_0_3 as tachys;

use std::sync::Arc;

use leptos::prelude::*;
use mf2::leptos::{RequestI18n, Setup, install};
use mf2::{
    ArgValue, Catalog, Compiled, Date, DateTime, Dir, Function, MsgId, Registry, Time, Tr, TrArgs,
    functions, markup, markup_key, tr, tr_args1, tr_rich,
};
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

fn kbd_handlers() -> Box<[(u64, Arc<dyn mf2::MarkupHandler>)]> {
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
fn a_string_is_isolated_and_a_plain_string_is_not() {
    let catalog = catalog_of("Hello, {$name}!", "en");
    let (html, string, from, plain, displayed) = in_request(&catalog, || {
        let d: TrArgs = tr_args1(ID, ArgValue::str_static("Ada"));
        (
            RenderHtml::to_html(d.clone()),
            d.to_string(),
            String::from(d.clone()),
            d.to_plain_string(),
            d.to_display_string(),
        )
    });
    let isolated = "Hello, \u{2068}Ada\u{2069}!";
    assert_eq!(html, isolated);
    // A message formatted as a single string gets the Default Bidi Strategy
    // by default (formatting.md; 04 §9, revised in Phase 7 A12)…
    assert_eq!(string, isolated);
    assert_eq!(from, isolated);
    assert_eq!(displayed, isolated);
    // …and the plain form is the one a program consumes.
    assert_eq!(plain, "Hello, Ada!");
}

/// `Display` pads the text the inherent `to_string()` builds, and `Debug`
/// shows the id and the arguments, as a derived `Debug` would (Phase 10:
/// owner question 14).
#[test]
fn display_is_the_inherent_to_string_and_debug_shows_the_arguments() {
    struct Opaque;
    impl mf2::CustomValue for Opaque {}

    let catalog = catalog_of("Hello, {$name}!", "en");
    let (string, shown, padded, via_trait) = in_request(&catalog, || {
        let d: TrArgs = tr_args1(ID, ArgValue::str_static("Ada"));
        (
            d.to_string(),
            format!("{d}"),
            format!("[{:<16}]", tr_args1(ID, ArgValue::str_static("Ada"))),
            ToString::to_string(&d),
        )
    });
    let isolated = "Hello, \u{2068}Ada\u{2069}!";
    assert_eq!(string, isolated);
    assert_eq!(shown, isolated);
    assert_eq!(via_trait, isolated);
    // 13 characters of text (the isolates count), padded to 16.
    assert_eq!(padded, format!("[{isolated}   ]"));

    let d: TrArgs = tr_args1(ID, ArgValue::str_static("Ada"));
    assert_eq!(
        format!("{d:?}"),
        "TrArgs { id: MsgId(0), args: [Str(\"Ada\")] }"
    );
    assert_eq!(format!("{:?}", tr(ID)), "Tr { id: MsgId(0) }");
    let rich = tr_rich(d, kbd_handlers());
    assert_eq!(
        format!("{rich:?}"),
        "TrRich { id: MsgId(0), args: [Str(\"Ada\")], handlers: 1 }"
    );
    assert_eq!(format!("{:?}", ArgValue::custom(Opaque)), "Custom(..)");
    assert_eq!(
        format!(
            "{:?}",
            tr_args1(
                ID,
                ArgValue::from(DateTime::floating(
                    Date::new(2026, 9, 28).expect("a date"),
                    Time::new(14, 5, 9, 7).expect("a time"),
                ))
            )
        ),
        "TrArgs { id: MsgId(0), args: [DateTime(DateTimeValue(2026-09-28T14:05:09.007))] }"
    );
    assert_eq!(
        format!(
            "{:?}",
            mf2::TrDyn::new(ID, [("n", ArgValue::from(2.5)), ("m", ArgValue::from(-3))])
        ),
        "TrDyn { id: MsgId(0), args: [(\"n\", Float(2.5)), (\"m\", Int(-3))] }"
    );
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

#[test]
fn the_switcher_is_a_get_form_applied_by_a_button() {
    // Phase 7 A11 (owner question 9): with no client code the form's own
    // `GET ?lang=` is the switch, so the markup alone must be complete — the
    // page's locale selected, the select named for `QueryParam`, a submit
    // button — and it must carry no fixed `id`, so a page may have two.
    use mf2::leptos::{LocaleOption, LocaleSwitcher};
    let catalog = catalog_of("Save", "ar");
    let html = in_request(&catalog, || {
        view! {
            <LocaleSwitcher label="Language" button="Apply">
                <LocaleOption tag="en">"English"</LocaleOption>
                <LocaleOption tag="ar">"العربية"</LocaleOption>
            </LocaleSwitcher>
        }
        .to_html()
    });
    let form_tag = &html[..html.find('>').expect("a tag")];
    assert!(form_tag.starts_with("<form "), "{html}");
    assert!(form_tag.contains(" method=\"get\""), "{html}");
    assert!(
        form_tag.contains(" class=\"mf2-locale-switcher\""),
        "{html}"
    );
    let label = html.find("<label>").expect("a label with no `for`");
    let select = html
        .find("<select name=\"lang\"")
        .expect("a select named `lang`");
    let label_end = html.find("</label>").expect("the label closes");
    assert!(
        label < select && select < label_end,
        "the select is inside its label: {html}"
    );
    assert!(html.contains("<span>Language</span>"), "{html}");
    assert!(
        html.contains("<button type=\"submit\">Apply</button>"),
        "{html}"
    );
    assert!(
        html.contains("<option value=\"en\" lang=\"en\">English</option>"),
        "{html}"
    );
    assert!(
        html.contains("<option value=\"ar\" lang=\"ar\" selected>العربية</option>"),
        "the page's locale is selected in the markup: {html}"
    );
    assert!(!html.contains(" id="), "no fixed id: {html}");
    assert!(!html.contains(" for="), "{html}");
}

/// A signal of any argument type is one (`IntoArg`): `u64` past `i64` and
/// `bool`, which 1.x's `ArgValue::from` did not take, read when the message
/// is formatted and not before.
#[test]
fn a_signal_of_any_argument_type_is_read_at_format_time() {
    use mf2::IntoArg;

    // The slots are in bytewise order: `$flag` before `$n`.
    let catalog = catalog_of(
        ".input {$flag :string} .match $flag true {{{$n :integer} on}} * {{{$n :integer} off}}",
        "en",
    );
    let (first, second) = in_request(&catalog, || {
        let n = RwSignal::new(u64::MAX);
        let flag = RwSignal::new(true);
        let d = mf2::tr_args2(ID, flag.into_arg(), n.into_arg());
        let first = d.to_plain_string();
        n.set(3);
        flag.set(false);
        (first, d.to_plain_string())
    });
    assert_eq!(first, "18446744073709551615 on");
    assert_eq!(second, "3 off");
}
