# Accessibility

WCAG 2.2 AA is a requirement of this library, not an option. Some of it the
library does for you. The rest is what any page has to do, and the
translated parts of a page make some of it easier to get wrong. This page
lists both. Its samples join the [call sites](call-sites.md) library.

## What the library does

* **The page's language (3.1.1).** `html_lang()` gives the shell
  `<html lang dir>` for the language the page is rendered in, and every
  switch updates both attributes. `dir` comes from the catalog, so a
  right-to-left language lays out right-to-left without a second
  stylesheet, as long as the CSS uses flexbox and logical properties
  (below).
* **The language of each option (3.1.2).** Each `<LocaleOption>` carries
  its own `lang` and names its language in that language, so a screen
  reader reads "Français" with a French voice.
* **No change of context on input (3.2.2).** The switcher applies a choice
  when its button is pressed, never on the `<select>`'s `change`, which the
  keyboard fires at every arrow key. See [Switching language](switching.md).
* **A labelled control (1.3.1, 3.3.2, 4.1.2).** The switcher's `<select>`
  is inside a visible `<label>` whose text you supply. It has no fixed
  `id`, so a page can have two switchers without duplicate ids.
* **Focus stays put.** A live switch rewrites text in place, so focus
  stays where it was, on the switcher's button. The one exception is a
  message with markup, whose fragment is built again in the new language:
  an element inside it (a link, say) is a new element after a switch.
* **Bidirectional text.** Arguments that need it (strings; not formatted
  numbers) are isolated in text a person reads, so a Latin name cannot
  scramble an Arabic sentence. The isolates are left
  out where a program reads the text. [Call sites](call-sites.md#attributes)
  lists which is which.
* **It works without the wasm.** A server-rendered page is complete, in the
  reader's language, before any client code runs. If the catalog cannot be
  loaded, the page stays as the server sent it instead of going blank.

## What the application does

* **Status messages (4.1.3).** Text that changes without moving focus,
  such as a count or a result, needs `role="status"` (or `aria-live`) so
  that it is announced. A signal-valued argument changes the text in
  place, so the same holds for it:

  ```mf2 file=calls/i18n/locales/en/main.mf2
  [results]
  count =
    .input {$n :integer}
    .match $n
    0   {{No results}}
    one {{One result}}
    *   {{{$n} results}}
  ```

  ```rust file=calls/src/lib.rs
  #[component]
  pub fn ResultCount(n: Signal<u32>) -> impl IntoView {
      view! { <p role="status">{tr!("results.count", n = n)}</p> }
  }
  ```

* **A title per page (2.4.2).** Give each route its own `<Title>` from a
  message. A client-side navigation then changes the title too.
* **Landmarks (1.3.1, 2.4.1).** Keep `<header>`, `<nav>`, `<main>` and
  `<footer>` as siblings, not nested inside `<main>`, so that "skip to
  main content" lands on the content.
* **Distinct labels (2.4.6).** Two fields with the same label are two
  messages that happen to say the same thing in English. Give each field
  its own message, even if the English text is the same, because another
  language may need to tell them apart.
* **Layout in both directions (1.4.10).** Lay out with flexbox, and use
  logical properties (`margin-inline-start`, `padding-inline`,
  `text-align: start`) instead of `left` and `right`. Then `dir="rtl"`
  mirrors the layout by itself. Leave room for text to grow: a German or
  Finnish label is often half as long again as the English one.
* **Contrast (1.4.3, 1.4.11).** Placeholder text and control borders are
  where the browsers' defaults fall below AA most often. Set them.

## Language as data

A page can state its language as structured data as well as in `<html
lang>`, for crawlers and for tools that read schema.org. Keep it in step
with a switch by reading `current_language` (from [Switching
language](switching.md#the-current-language)) in a closure:

```mf2 file=calls/i18n/locales/en/main.mf2
[article]
headline = How the catalog is built
```

```rust file=calls/src/lib.rs
#[component]
pub fn Article() -> impl IntoView {
    view! {
        <article itemscope itemtype="https://schema.org/Article">
            <meta itemprop="inLanguage" content=current_language />
            <h2 itemprop="headline">{tr!("article.headline")}</h2>
        </article>
    }
}
```

## Untranslated text

If a message has not been translated yet, `missing = "fallback"` (the
default in `mf2.toml`) shows the source language's text in its place.
Other options are `"id"` (the message's id, so the gap is visible) and
`"empty"`. That borrowed text is in a different language from the page,
and WCAG 3.1.2 asks for such a passage to be marked with its own `lang`.
Turn on `mark-fallback-lang` and the library does it:

```toml file=calls/Cargo.toml merge
[dependencies]
leptos-mf2 = { version = "1", features = ["mark-fallback-lang"] }
```

A message the page's catalog borrowed then renders inside a `<span>`
naming the language it came from. In an Arabic page, an English sentence
becomes `<span lang="en" dir="ltr">…</span>`: a screen reader reads it
with an English voice, and it lays out left to right. `dir` is added only
when the two languages' directions differ. A translated message is still
a bare text node, so the feature changes nothing on a page with no
missing translation. The server writes the span, hydration keeps the one
it wrote, and a live switch adds or removes it around the same text.

Some places cannot be marked, and stay as they are:

* **Attributes** (`title`, `aria-label`, `alt`, `placeholder`). HTML
  gives an attribute a language only through its element's `lang`, which
  would relabel the element's content as well.
* **Strings** (`to_string()`, `String::from`, `TextProp`,
  `Signal<String>`). A string has no markup to carry a `lang`.
* **Elements that hold only text** (`<title>`, `<textarea>`, `<option>`,
  `<script>`, `<style>`). A span there would be shown as characters, so a
  borrowed message in one is written without it, on the server and in the
  browser.

For those, the answer is to translate the message. Either way:

* `mf2 -C i18n check` warns about every missing translation
  (`missing-translation`), and `mf2 -C i18n stats` counts them per
  language;
* to make a missing translation fail the build, raise the lint in
  `mf2.toml`:

  ```toml file=calls/i18n/mf2.toml merge
  [lints]
  missing-translation = "error"
  ```

## How the examples are checked

The examples in this repository ([`examples/demo-ssr`](https://github.com/EvanCarroll/rust-mf2/tree/main/examples/demo-ssr),
[`demo-islands`](https://github.com/EvanCarroll/rust-mf2/tree/main/examples/demo-islands),
[`demo-csr`](https://github.com/EvanCarroll/rust-mf2/tree/main/examples/demo-csr)) are audited against WCAG 2.2 AA in
every language, right to left included.
[`tools/e2e/checks/a11y.mjs`](https://github.com/EvanCarroll/rust-mf2/blob/main/tools/e2e/checks/a11y.mjs) runs axe-core
over 28 pages in Chromium and Firefox and finds no violations. It also
checks contrast, reflow at 320 CSS pixels, text spacing, landmarks, and the
switcher from the keyboard. The audit and its findings are in
[`plans/phase-7-results.md`](https://github.com/EvanCarroll/rust-mf2/blob/main/plans/phase-7-results.md). It has not been done with a screen reader; the
browsers' accessibility tree stands in for one.
