# Call sites: `tr!` in every position

There is one macro. `tr!("id", name = value, …)` works in a text node, an
attribute, a component prop, a `const` table, and a `String`, and it costs
each call site about the same in the wasm wherever it is used. This page
shows each position, with the messages it uses.

The samples on this page are components in one library, which uses the
translation crate from [Getting started](getting-started.md).
`cargo xtask docs` compiles them for the server and for the browser. They
begin with:

```rust file=calls/src/lib.rs
use hello_i18n::tr;
use leptos::prelude::*;
use leptos_mf2::{ArgValue, DateTimeValue, Tr};
```

and their messages begin with:

```mf2 file=calls/i18n/locales/en/main.mf2
@locale en
---

welcome = Welcome back
```

## What `tr!` returns

`tr!` does not produce text. It produces a **description** of a message: the
message's number and its arguments. The text is made where the description
is used, in whichever language is current. This is why one macro works in
every position, and why text rendered from a description follows a
language switch without a closure at the call site.

The macro checks the call against the messages **at compile time**:

* the id exists (if it is misspelt, the error suggests the closest one);
* the arguments are exactly the message's variables: none missing, none
  unknown, none given twice;
* if markup handlers are given, there is one for every markup element of
  the message, and none for an element it does not have;
* each argument's type converts to a message argument.

A message with no arguments is a `Tr`: `Copy`, four bytes, and usable in a
`const`. A message with arguments is a `TrArgs`. One with markup handlers is
a `TrRich`. Nothing at the call site carries text, an id, or an argument
name. Arguments are passed by position, and the names stay in the build.

## Text

```rust file=calls/src/lib.rs
#[component]
pub fn Welcome() -> impl IntoView {
    view! { <h1>{tr!("welcome")}</h1> }
}
```

In the browser the text node joins the library's registry of nodes. On a
language switch, the registry rewrites every registered node directly. No
effect runs per call site, and the node's slot is freed when it unmounts.

## Attributes

A description can be the value of any attribute:

```mf2 file=calls/i18n/locales/en/main.mf2
[search]
placeholder = Search the catalog
label = Search
```

```rust file=calls/src/lib.rs
#[component]
pub fn Search() -> impl IntoView {
    view! {
        <input
            type="search"
            placeholder=tr!("search.placeholder")
            aria-label=tr!("search.label")
        />
    }
}
```

**Bidi isolation is chosen by the attribute's name.** An argument inside a
message is wrapped in invisible Unicode isolates (U+2066–U+2069). The MF2
specification makes this the default so that, for example, a Latin name in
an Arabic sentence cannot reorder the sentence around it. Whether the marks
belong in the text depends on who reads it. For a person they are right;
for a program they are junk:

| The attribute | Its text |
|---|---|
| `value`, `href`, `src`, `srcset`, `action`, `formaction`, `poster`, `cite`, `download`, `id`, `name`, `for`, `form`, `list`, `class`, `type`, and every `data-*` | **plain**: a program reads it (a form submission, a URL, a script) |
| every other name: `title`, `alt`, `aria-*`, `placeholder`, `label`, `content`, … | **isolated**: a person reads it |

The name match ignores ASCII case, and the rule is the same on the server
and in the browser. `prop:value=` (a DOM property) is always plain:

```mf2 file=calls/i18n/locales/en/main.mf2
[signature]
label = Signature
text = Signed, {$name}
```

```rust file=calls/src/lib.rs
#[component]
pub fn Signature(name: String) -> impl IntoView {
    view! {
        <label>
            {tr!("signature.label")}
            // Submitted with the form: no marks in `value`. Read by a person:
            // the name is isolated in `title`.
            <input
                name="signature"
                value=tr!("signature.text", name = name.clone())
                title=tr!("signature.text", name = name)
            />
        </label>
    }
}
```

## Component props

A description converts into what a component takes text as: `TextProp`,
`Signal<String>`, `Oco<'static, str>` and `String`. Prefer
`#[prop(into)] TextProp`. It is *derived*, so the component re-reads it
after a language switch:

```mf2 file=calls/i18n/locales/en/main.mf2
[settings]
title = Settings
intro = Choose how the catalog looks to you.
```

```rust file=calls/src/lib.rs
#[component]
pub fn Card(#[prop(into)] title: TextProp, children: Children) -> impl IntoView {
    view! {
        <section>
            <h2>{move || title.get()}</h2>
            {children()}
        </section>
    }
}

#[component]
pub fn Settings() -> impl IntoView {
    view! {
        <Card title=tr!("settings.title")>
            <p>{tr!("settings.intro")}</p>
        </Card>
    }
}
```

| A prop of type | After a language switch |
|---|---|
| `TextProp`, `Signal<String>` | follows it (derived) |
| `Oco<'static, str>`, `String` | keeps the text it had: it is a value |

On the server, a derived prop captures the request's catalog when it is
converted. Code that reads the prop after rendering, such as `leptos_meta`
reading `<Title text=…>`, therefore still gets the request's language.

## Arguments

Arguments are named at the call site. They can be literals, variables or
expressions:

```mf2 file=calls/i18n/locales/en/main.mf2
[order]
summary = {$customer}: {$items :integer} items, {$total :currency currency=EUR}
exact = Exactly {$amount :number minimumFractionDigits=2}
```

```rust file=calls/src/lib.rs
#[component]
pub fn Order(customer: String, items: u32, total: f64) -> impl IntoView {
    view! {
        <p>{tr!("order.summary", customer = customer, items = items, total = total)}</p>
        // A decimal as its exact text, not rounded through f64.
        <p>{tr!("order.exact", amount = ArgValue::decimal("19.99"))}</p>
    }
}
```

| From | Is |
|---|---|
| `&str`, `String`, `&String`, `Arc<str>`, `char` | a string. A literal's `&'static str` is kept as it is; other text is counted, not copied again |
| `i8`…`i64`, `u8`…`u32`, `usize` | an integer |
| `f32`, `f64` | a floating-point number |
| `ArgValue::decimal("19.99")` | an exact decimal, as its text |
| `DateTimeValue` | a date and time (below) |
| a signal: `Signal`, `ReadSignal`, `RwSignal`, `Memo` (and their `Arc` forms) | its value, read when the message is formatted (below) |

### Dates

A date needs the `fn-datetime` feature and a backend (`datetime-icu`, or
`datetime-intl` for the browser's own formatter) on the translation crate:

```mf2 file=calls/i18n/locales/en/main.mf2
[post]
published = Published {$when :datetime dateLength=long}
```

```rust file=calls/src/lib.rs
#[component]
pub fn Published(epoch_ms: i64) -> impl IntoView {
    // `instant` is `None` past the years a date can hold.
    let when = DateTimeValue::instant(epoch_ms);
    view! { <p>{when.map(|when| tr!("post.published", when = when))}</p> }
}
```

The options are MessageFormat 2's, not JavaScript's: `dateFields`,
`dateLength` and `timePrecision` on `:datetime`, `fields` and `length` on
`:date`, `precision` on `:time`, and `timeZoneStyle` to show the zone. An
option a function does not have is ignored when the message formats, so
`dateStyle=long` gives the default length; `mf2 check` warns about it
(`unknown-option`).

**Dates are shown in the reader's time zone**, with no code in the
application:

* In the browser, the library asks for the reader's zone
  (`Intl.DateTimeFormat().resolvedOptions().timeZone`).
* A server cannot know it on a reader's **first visit**, so that page is
  rendered in UTC — or in the zone `setup().with_time_zone(…)` names. When
  the page has hydrated, the library rewrites the dates that come out
  differently in the reader's zone, and only those; the rest of the page is
  not touched. It then remembers the zone in a cookie, `mf2_tz`.
* **Every later page** is rendered in the reader's zone from the start:
  `mf2-axum` reads the cookie, and the page says which zone it was rendered
  in, so nothing changes after hydration. A zone the server's time zone
  database does not know, or a malformed cookie, is ignored.
* A **client-only** application renders in the reader's zone from its
  first frame, and writes no cookie.

So a reader on a first visit may see a date change once, just after the
page becomes interactive. A message that must show one particular zone —
an event's local time, say — names it, and the reader's zone does not
apply:

```mf2 file=calls/i18n/locales/en/main.mf2
starts = Doors open {$when :time timeZone=|Europe/Paris| timeZoneStyle=short}
```

The zone, in order: the one the message names (`timeZone=input` means the
value's own, and an instant's own zone is UTC unless it was given one);
else the reader's, once known; else `with_time_zone`'s; else UTC. A
floating value (`DateTimeValue::floating`) is a wall time with no zone and
is shown as it is.

A value can carry a zone of its own, for `timeZone=input` to show:
`DateTimeValue::instant(t).with_zone("Europe/Paris")` is the instant `t`,
shown at Paris's wall time; `DateTimeValue::wall_time(date, time,
"Europe/Paris")` is that wall time in Paris, whatever instant it is.

**Islands.** Dates inside islands are corrected like any others. A date in
a component that stays on the server (not an island) is not sent to the
browser as code, so it cannot be corrected: on a reader's first visit it
stays in UTC (or `with_time_zone`'s zone) until the next page, which the
cookie renders in the reader's zone.

### Arguments that change

Pass a signal and the text follows it. The call site has no closure: the
library runs one effect for the node, and that effect reads the signal.

```mf2 file=calls/i18n/locales/en/main.mf2
[cart]
items =
  .input {$count :integer}
  .match $count
  0   {{Your cart is empty}}
  one {{One item in your cart}}
  *   {{{$count} items in your cart}}
add = Add an item
```

```rust file=calls/src/lib.rs
#[component]
pub fn Cart() -> impl IntoView {
    let count = RwSignal::new(0u32);
    view! {
        <p role="status">{tr!("cart.items", count = count)}</p>
        <button on:click=move |_| *count.write() += 1>{tr!("cart.add")}</button>
    }
}
```

For a value computed from other signals, pass a `Memo` or a
`Signal::derive(…)`. A signal that has been disposed reads as unset. The
message then reports an unresolved variable instead of panicking.

## Choosing between messages

A closure can return a different message depending on state. Every branch
must return the same type: `Tr` for messages without arguments, `TrArgs`
for messages with them.

```mf2 file=calls/i18n/locales/en/main.mf2
[status]
online = Online
offline = Offline
```

```rust file=calls/src/lib.rs
#[component]
pub fn Status(online: Signal<bool>) -> impl IntoView {
    view! {
        <p>
            {move || if online.get() { tr!("status.online") } else { tr!("status.offline") }}
        </p>
    }
}
```

## Markup as elements

MF2 messages can contain markup: `{#name}…{/name}`. A call site turns each
markup element into a real element with a closure. The message decides
where the element goes, so a translation can move it to the place its
grammar needs, and the view does not have to know the language's word
order:

```mf2 file=calls/i18n/locales/en/main.mf2
[terms]
accept = By continuing you accept our {#link}terms of use{/link} and {#strong}our privacy policy{/strong}.
```

```rust file=calls/src/lib.rs
#[component]
pub fn Terms() -> impl IntoView {
    view! {
        <p>
            {tr!(
                "terms.accept",
                link = |children: AnyView| view! { <a href="/terms">{children}</a> },
                strong = |children: AnyView| view! { <strong>{children}</strong> },
            )}
        </p>
    }
}
```

Give a handler for **every** markup element of the message, or for none.
Leaving one out is a compile error, because it is almost always an
oversight. The element structure comes from the catalog, so the page waits
for the catalog before it hydrates. Every client entry point on the
[delivery modes](delivery-modes.md) page does this.

## Strings

In code that needs text rather than a view, such as a toast, an error
value, a server function argument or `format!`, turn the description into a
`String`:

```mf2 file=calls/i18n/locales/en/main.mf2
[file]
saved = Saved {$name}
default-name = Untitled {$n :integer}
```

```rust file=calls/src/lib.rs
/// Text a person will read: isolated, as the MF2 specification requires of
/// a message formatted to a single string.
pub fn saved_notice(name: &str) -> String {
    tr!("file.saved", name = name).to_string()
}

/// Text a program will read (a file name, a comparison, the clipboard):
/// no invisible bidi marks in it.
pub fn default_file_name(n: u32) -> String {
    tr!("file.default-name", n = n).to_plain_string()
}
```

| Method | Isolated? | For |
|---|---|---|
| `to_string()`, `String::from(…)` | yes | text a person reads |
| `to_plain_string()` | no | text a program reads |

`to_display_string()` still exists as another name for `to_string()`.

These read the catalog in force when they are called. In the browser,
calling one inside a closure subscribes that closure to the language, so
it re-runs after a switch. A **text node** is always isolated, because it
has no attribute name to decide by. When a text node must be plain (for
example the starting text of a `<textarea>`), use a closure:

```mf2 file=calls/i18n/locales/en/main.mf2
[draft]
body = Dear {$name},
```

```rust file=calls/src/lib.rs
#[component]
pub fn Draft(name: String) -> impl IntoView {
    view! {
        <textarea name="body">
            {move || tr!("draft.body", name = name.clone()).to_plain_string()}
        </textarea>
    }
}
```

Such a closure costs more than a text node: in the churn benchmark
(`cargo xtask churn`), a mounted row holding one uses about 546 bytes of
heap, against about 113 for a row holding a text node. Use it only where
plain text is needed.

## Messages as data

A `Tr` is a constant, so a table of commands, menu entries or errors can
hold messages as plain data and format them when they are shown:

```mf2 file=calls/i18n/locales/en/main.mf2
[command]
pause = Pause
resume = Resume
```

```rust file=calls/src/lib.rs
pub struct Command {
    pub name: &'static str,
    pub label: Tr,
}

pub const COMMANDS: &[Command] = &[
    Command { name: "pause", label: tr!("command.pause") },
    Command { name: "resume", label: tr!("command.resume") },
];

#[component]
pub fn Commands() -> impl IntoView {
    view! {
        <ul>
            {COMMANDS
                .iter()
                .map(|command| view! { <li data-command=command.name>{command.label}</li> })
                .collect_view()}
        </ul>
    }
}
```

## When the id is only known at run time

`tr!` needs the id and the argument names when it is compiled. A tool that
formats messages chosen by data (a preview, a test fixture, a server
formatting a message named in a request) can use `hello_i18n::msg_id!("id")`
and `TrDyn`. `TrDyn` carries argument names and matches them at run time.
It is not for the browser: it puts names in the wasm, which is what `tr!`
exists to avoid.
