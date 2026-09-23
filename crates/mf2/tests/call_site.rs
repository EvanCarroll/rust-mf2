//! The call-site core (`plans/04-leptos-integration.md` §2.1): what `tr!`
//! builds, formatted against a catalog the caller supplies — no Leptos, no
//! ambient state.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use mf2::{
    ArgSource, ArgValue, Compiled, CustomValue, Date, DateTimeValue, FormatContext, Formatter,
    Function, Handler, MarkupHandler, Measure, MeasureUnit, MsgId, Number, Registry, Time, Tr,
    functions, markup, tr, tr_args_n, tr_args1, tr_args2, tr_dyn, tr_rich,
};

static FUNCTIONS: [(&str, &dyn Function); 4] = [
    ("integer", &functions::INTEGER),
    ("number", &functions::NUMBER),
    ("offset", &functions::OFFSET),
    ("string", &functions::STRING),
];
/// Unannotated date/time values need the hook the closed-world registry
/// gets when `fn-datetime` is on — without it they are a Bad Operand, which
/// is the documented default-configuration degradation (L4d).
static REGISTRY: Registry = Registry::new(&FUNCTIONS).with_dates(&mf2::fn_datetime::DATES);
static CX: FormatContext = FormatContext::new(&mf2::host_std::HOST);

/// A message compiled for `en`, and the slot order its manifest fixed.
fn compiled(src: &str) -> Compiled {
    mf2::compile_str(src, "en").expect("the message compiles")
}

fn slots(c: &Compiled) -> Vec<String> {
    c.manifest.slots.first().cloned().unwrap_or_default()
}

fn text(c: &Compiled, f: impl FnOnce(&Formatter<'_>) -> String) -> String {
    let formatter = Formatter::new(&c.catalog, &REGISTRY, &CX);
    f(&formatter)
}

#[test]
fn a_call_site_without_arguments_is_four_bytes_and_const() {
    const HOME: Tr = tr(MsgId::from_raw(0));
    assert_eq!(size_of::<Tr>(), 4);
    assert_eq!(align_of::<Tr>(), 4);
    assert_eq!(HOME.id(), Compiled::ID);
    // `Copy`, so a description can sit in a `const` table and be handed
    // around without a clone.
    let copy = HOME;
    assert_eq!(copy.id().raw(), HOME.id().raw());

    let c = compiled("Save");
    assert_eq!(text(&c, |f| HOME.format(f)), "Save");
}

#[test]
fn arguments_go_in_slot_order_and_format() {
    let c = compiled("Hello, {$name}! You are {$rank :integer}.");
    // The manifest's slot order is bytewise ascending, not the order the
    // call site wrote: `name` before `rank`.
    assert_eq!(slots(&c), ["name", "rank"]);
    let description = tr_args2(Compiled::ID, ArgValue::from("Ada"), ArgValue::from(3));
    // The Default Bidi Strategy isolates every placeholder (U+2066–U+2069),
    // so that an interpolated name cannot scramble the sentence.
    assert_eq!(
        text(&c, |f| description.format(f)),
        "Hello, \u{2068}Ada\u{2069}! You are 3."
    );
}

#[test]
fn a_missing_value_is_an_unresolved_variable_not_a_panic() {
    let c = compiled("Hello, {$name}!");
    let description = tr_args1(Compiled::ID, ArgValue::Unset);
    let formatter = Formatter::new(&c.catalog, &REGISTRY, &CX);
    let mut out = String::new();
    let mut errors = Vec::new();
    description.write(&formatter, &mut out, &mut errors);
    assert_eq!(out, "Hello, \u{2068}{$name}\u{2069}!");
    assert_eq!(errors.len(), 1);
}

#[test]
fn five_arguments_spill_to_the_heap_and_stay_in_order() {
    let c = compiled("{$a} {$b} {$c} {$d} {$e}");
    assert_eq!(slots(&c), ["a", "b", "c", "d", "e"]);
    let description = tr_args_n(
        Compiled::ID,
        vec![
            ArgValue::from(1),
            ArgValue::from(2),
            ArgValue::from(3),
            ArgValue::from(4),
            ArgValue::from(5),
        ]
        .into_boxed_slice(),
    );
    assert_eq!(description.args().len(), 5);
    assert_eq!(text(&c, |f| description.format(f)), "1 2 3 4 5");
}

#[test]
fn a_source_is_read_once_per_format_and_at_format_time() {
    /// What `leptos-mf2` does with a signal: read it when the formatter
    /// asks, which is inside the reactive context that is rendering.
    struct Counter(AtomicU32);

    impl ArgSource for Counter {
        fn arg_value(&self) -> ArgValue {
            ArgValue::Int(i64::from(self.0.fetch_add(1, Ordering::Relaxed)))
        }
    }

    let counter = Arc::new(Counter(AtomicU32::new(7)));
    let c = compiled("{$n :integer}");
    let description = tr_args1(Compiled::ID, ArgValue::Source(counter.clone()));
    // Nothing was read when the call site was built.
    assert_eq!(counter.0.load(Ordering::Relaxed), 7);
    assert_eq!(text(&c, |f| description.format(f)), "7");
    assert_eq!(text(&c, |f| description.format(f)), "8");
    assert_eq!(counter.0.load(Ordering::Relaxed), 9);
}

#[test]
fn a_source_that_returns_a_source_cannot_loop() {
    struct Itself;

    impl ArgSource for Itself {
        fn arg_value(&self) -> ArgValue {
            ArgValue::Source(Arc::new(Itself))
        }
    }

    let c = compiled("{$n}");
    let description = tr_args1(Compiled::ID, ArgValue::source(Itself));
    // It resolves to nothing, which is an Unresolved Variable — the same
    // outcome as `Unset`, and not a hang.
    assert_eq!(text(&c, |f| description.format(f)), "\u{2068}{$n}\u{2069}");
}

#[test]
fn a_date_argument_is_borrowed_into_the_runtime() {
    let c = compiled("{$when}");
    let date = Date::new(2026, 9, 23).expect("a real date");
    let time = Time::new(14, 5, 0, 0).expect("a real time");
    let value = DateTimeValue::floating(date, time);
    let description = tr_args1(Compiled::ID, ArgValue::from(value));
    // What the date *looks* like is the backend's business — the stub, ICU4X
    // and the browser all differ — so this asserts only what the call-site
    // core is responsible for: the value reached the date functions through
    // `Arg::DateTime`, which takes it by reference, so the borrow held.
    let floating = text(&c, |f| description.format(f));
    assert!(floating.contains("2026"), "the date resolved: {floating:?}");
    assert!(!floating.contains("{$when}"), "{floating:?}");

    // The instant constructor carries the offset, so the epoch is 1970 in
    // UTC…
    let instant = DateTimeValue::instant(0).expect("the epoch is a real instant");
    let utc = text(&c, |f| {
        tr_args1(Compiled::ID, ArgValue::from(instant.clone())).format(f)
    });
    assert!(utc.contains("1970"), "{utc:?}");

    // …and the zone name a call site owns survives the borrow: asked to show
    // the value in its own zone, the same instant is another wall clock
    // (datetime.md, `timeZone=input`).
    let own = compiled("{$when :datetime timeZone=input}");
    let sydney = instant.with_zone("Australia/Sydney");
    let description = tr_args1(Compiled::ID, ArgValue::from(sydney));
    assert_ne!(text(&own, |f| description.format(f)), utc);
}

#[test]
fn an_application_value_reaches_a_function_as_its_measure() {
    /// An application's own money type — the `Custom` variant's reason to
    /// exist.
    struct Money(i64);

    impl CustomValue for Money {
        fn as_number(&self) -> Option<Number> {
            Some(Number::from_i64(self.0))
        }

        fn as_measure(&self) -> Option<Measure<'_>> {
            Some(Measure::new(
                Number::from_i64(self.0),
                MeasureUnit::Currency(*b"EUR"),
                0,
            ))
        }
    }

    let c = compiled("{$amount :integer}");
    let description = tr_args1(Compiled::ID, ArgValue::custom(Money(42)));
    assert_eq!(text(&c, |f| description.format(f)), "42");
}

#[test]
fn a_rich_call_site_finds_its_handler_by_the_name_the_catalog_gives() {
    /// A handler stands in for `leptos-mf2`'s: the core only carries it.
    struct Element(&'static str);

    impl MarkupHandler for Element {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let c = compiled("Press {#kbd}Esc{/kbd} to {#b}close{/b}");
    let description = tr_rich(
        mf2::tr_args0(Compiled::ID),
        vec![
            (mf2::markup_key("b"), markup(Handler(Element("b")))),
            (mf2::markup_key("kbd"), markup(Handler(Element("kbd")))),
        ]
        .into_boxed_slice(),
    );

    let found = description.handler("kbd").expect("kbd has a handler");
    let found = found
        .as_any()
        .downcast_ref::<Element>()
        .expect("the renderer knows its own type");
    assert_eq!(found.0, "kbd");
    assert!(description.handler("i").is_none());

    // Markup contributes no text, so a rich description's string is the
    // message's text — handlers or not.
    assert_eq!(text(&c, |f| description.format(f)), "Press Esc to close");
}

#[test]
fn text_keeps_a_literal_static_and_shares_everything_else() {
    // A literal costs no allocation…
    let ArgValue::Str(mf2::Text::Static(s)) = ArgValue::str_static("Ada") else {
        panic!("a literal stays static");
    };
    assert_eq!(s, "Ada");
    // …and a cloned description shares the counted text rather than copying
    // it, which is what a re-format after a locale change does.
    let owned = ArgValue::from(String::from("Ada"));
    let ArgValue::Str(mf2::Text::Shared(a)) = owned.clone() else {
        panic!("an owned string is counted");
    };
    let ArgValue::Str(mf2::Text::Shared(b)) = owned else {
        panic!("an owned string is counted");
    };
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn named_arguments_may_deliberately_mismatch_the_message() {
    struct Always;

    impl ArgSource for Always {
        fn arg_value(&self) -> ArgValue {
            ArgValue::str_static("Grace")
        }
    }

    let c = compiled("Hello, {$name}!");

    // What the suite's `dyn` tests do: pass a name the message does not
    // declare. It is ignored, and the variable nothing matched is an
    // Unresolved Variable with its fallback text.
    let mismatched = tr_dyn(
        Compiled::ID,
        vec![(mf2::Text::Static("other"), ArgValue::from("x"))],
    );
    assert_eq!(
        text(&c, |f| mismatched.format(f)),
        "Hello, \u{2068}{$name}\u{2069}!"
    );

    // And the matching case, by name rather than by slot.
    let matched = tr_dyn(
        Compiled::ID,
        vec![(mf2::Text::Static("name"), ArgValue::from("Ada"))],
    );
    assert_eq!(
        text(&c, |f| matched.format(f)),
        "Hello, \u{2068}Ada\u{2069}!"
    );

    // The same lowering as the positional path: a source is read here too.
    let from_source = tr_dyn(
        Compiled::ID,
        vec![(mf2::Text::Static("name"), ArgValue::source(Always))],
    );
    assert_eq!(
        text(&c, |f| from_source.format(f)),
        "Hello, \u{2068}Grace\u{2069}!"
    );
}
