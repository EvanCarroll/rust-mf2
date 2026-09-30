# Troubleshooting

Each entry gives what you see, why, and what to change. The library says
something wherever it used to fall back without a word: the build fails
with a message, a native application panics, a server writes one line to
standard error, or a browser debug build writes one line to the console.
A server says each thing once per process (once per language, where it
names one), not once per request.

## `tr!` is not found

**You see**, in a module of the crate whose build script compiles the
messages:

```text
error: cannot find macro `tr` in this scope
   --> core/src/report.rs:2:5
    |
  2 |     tr!("title", target = "example.org")
    |     ^^
    |
help: consider importing this macro through its public re-export
    |
  1 + use crate::tr;
    |
```

(rustc may also offer `r#try!` as a similar name; it is not the fix.)

**Why:** the generated module makes `tr` an ordinary item of the crate's
root, so it is in scope at the root, after `mf2::include_generated!()`,
and nowhere else. Every other module imports it, whether it is declared
before the include or after it.

**Fix:** import it once per module:

* in the crate that includes the generated module, `use crate::prelude::*;`
  (which also brings `Locale`, `set_locale` and the rest), `use crate::tr;`,
  or write `crate::tr!(…)`;
* in a crate that depends on it, `use my_lib::prelude::*;`,
  `use my_lib::tr;`, or `my_lib::tr!(…)`.

Do not define a `macro_rules! tr` of your own beside the generated one:
two items named `tr` make every call ambiguous.

## A stale manifest

**You see** a `tr!` that fails to compile although its message exists,
with this error (the path and the two hashes are your build's):

```text
the message manifest at …/out/manifest.mf2m is stale: it hashes to 0x…, but this `tr!` was generated for 0x…
the generated module and the manifest are from different builds — rebuild the i18n crate (in an editor: restart the proc-macro server)
```

or, when the manifest is missing:

```text
cannot read the message manifest at …/out/manifest.mf2m: No such file or directory (os error 2)
the i18n crate's build script writes it — build that crate, or, if its target directory moved, rebuild it there
```

**Why:** `tr!` checks each call against the manifest the build script
wrote, and the generated module records which manifest that was. An editor
usually causes this: its proc-macro server keeps an older build's module
while `cargo` has written a newer manifest, or the target directory was
moved or cleaned.

**Fix:** run `cargo build` for the crate that holds the messages. In an
editor, restart its proc-macro server (in rust-analyzer: *Restart server*).

**On the web**, the same mismatch at run time is a deploy skew: a page
from one build loaded a catalog from another. The client does not use the
catalog; it writes `mf2: this page's catalog is from another deploy;
reloading.` to the console and reloads. If it keeps happening, the server
or a cache is serving the wasm and the catalogs from different deploys:
publish them together.

## Empty text

**A native application** panics at its first message:

```text
mf2: no catalogs are installed: call install() at start-up
```

**Why:** nothing loaded the catalogs. **Fix:** call the generated
`install()` first thing in `main`. A test needs no `install()`:
[`with_locale`](testing.md#one-language-for-one-test-with_locale) loads the
embedded catalogs itself.

**A server** renders the page with every message empty, and writes once:

```text
mf2: a message was formatted with no catalogs installed, so it rendered as empty text; call the generated install() at start-up
```

**Fix:** call `install()` in the server's `main`, before the router is
built.

**In the browser**, text is empty when a component formats a message
before any catalog is active. A debug build says so once in the console:

```text
mf2: a message was formatted before any catalog was active, so it rendered as empty text; start the page through mf2::leptos (hydrate_body, mount_to_body), which loads the catalog first. (Debug builds only.)
```

A release build has no such code, so check a debug build first.
**Fix:** start the client as
[Getting started](getting-started.md#the-page) does, through
`mf2::leptos`'s `hydrate_body` or `mount_to_body`, which load the catalog
before the first render.

## Pages stuck in the default language

The page renders, always in the source language. The server says why,
once:

* **No language for the request:**

  ```text
  mf2: a page rendered without the request's language, so it is in the source language, `en`; add mf2::axum's Negotiator layer to the router, or call provide_locale in the render
  ```

  A request's render ran outside the negotiator (the route list the
  server builds at start-up never prints it). **Fix:** add the `Negotiator`
  layer to the router that serves the pages, as
  [Getting started](getting-started.md#the-server) does, or call
  `provide_locale` in the render.

* **A language the build does not have:**

  ```text
  mf2: provide_locale("pt-BR"): this build has no catalog for that language, so the page renders in the source language, `en`
  ```

  **Fix:** add a catalog for it (`locales/pt-BR/`), or pass a tag the
  application ships; `Locale::ALL` lists them.

* **The reader's languages match no catalog:**

  ```text
  mf2: no catalog matches the reader's languages (zh-TW, zh), so they are served the default language, `en`; a catalog for one of them would serve them
  ```

  This is a normal outcome, and the line tells you which catalog is
  missing. A Traditional Chinese reader is a common case: Traditional and
  Simplified Chinese do not fall back to each other, as CLDR's data says,
  so a `zh-Hans` catalog does not serve `zh-TW`. **Fix:** add a `zh-Hant`
  catalog ([`[fallback]`](configuration.md#fallback) says how the chains
  are built).

* **The language is in the path, and the redirect sends everyone to
  `?lang`:**

  ```text
  mf2: path_prefix_redirect ran on a request the Negotiator has not seen, so it reads the query parameter `lang`; add its .layer before the negotiator's, so that it runs under it
  ```

  **Fix:** in axum the last `.layer` runs first, so add the redirect's
  `.layer` before the `Negotiator`'s
  ([Switching language](switching.md#how-the-server-chooses)).

If the server says nothing and the page is still in the source language,
check the switcher first: [Switching language](switching.md) describes
how the choice is remembered and sent.

## A test sees the wrong language

A test that calls `set_locale` changes the language of every thread, so
the tests that run beside it see it too. Use
[`with_locale`](testing.md#one-language-for-one-test-with_locale), which
holds for one closure on one thread, and format inside the closure: a
description made inside `with_locale` and turned into text after it is in
the thread's own language again.
