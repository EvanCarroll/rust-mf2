# Rust MF2

Unicode MessageFormat 2 (MF2) for Rust applications: web applications
with [Leptos](https://leptos.dev), and native command-line and terminal
applications, including Ratatui. Translations are written in MF2 and
checked when the application compiles, and each language becomes one
small binary catalog. On the web, the browser downloads a language's
catalog when it needs it, a language switch is live, and the wasm contains
none of the text. A native application embeds its catalogs or ships them
beside the executable, and follows the system's language.

| Page | What it covers |
|---|---|
| [How the crates fit together](ecosystem.md) | which crate does what, and which ones an application names |
| [Getting started](getting-started.md) | installing, the manifest and build script, the messages, a server-rendered application that hydrates and switches language live |
| [MF2 for developers](mf2-for-developers.md) | the `.mf2` file, placeholders, functions, plurals and ordinals, gender, markup, notes for translators; MF2 beside Fluent, ICU MessageFormat 1 and i18next; why messages have ids |
| [Call sites](call-sites.md) | `tr!` in text, attributes, props and strings; arguments, signals, dates; markup as elements; plain and isolated text |
| [Delivery modes](delivery-modes.md) | SSR + hydrate, lazy routes, islands, client-only |
| [Switching language](switching.md) | how the server chooses, the switcher, what a switch does, your own control |
| [Native CLI and Ratatui apps](native-apps.md) | `mf2-native` for a command-line or terminal application: embedded or shipped catalogs, the system's language, Ratatui text with markup as styles |
| [The command line](command-line.md) | every `mf2` command: `check`, `fmt`, `compile`, `stats`, `dump`, `export` and `import` (JSON, XLIFF 2), `pseudo`, `watch`, `convert --from fluent` and its report codes |
| [Translating](translating.md) | a round of translation: XLIFF 2 for a translation tool, JSON for a review, an import that carries a mistake, pseudo-locales, `mf2 stats`, and the checks for CI |
| [Testing](testing.md) | `with_locale` in tests, the text in each language, Ratatui frames on `TestBackend`, pseudo-locales for the layout |
| [Troubleshooting](troubleshooting.md) | `tr!` not found, a stale manifest, empty text, pages stuck in the default language: what you see, why, the fix |
| [`mf2.toml`](configuration.md) | every key: the source language, fallback chains, what a catalog carries, lint levels, the application's own functions |
| [Lints](lints.md) | every check the build and `mf2 check` make: what raises it, an example, the fix, its default level |
| [Features of `mf2`](features.md) | every feature: the Leptos line and modes, a server, native applications, functions and their backends, hosts; what text costs in a browser build |
| [Accessibility](accessibility.md) | what the library does for WCAG 2.2 AA, and what the application does |
| [Migrating from `leptos-fluent`](migrating-from-leptos-fluent.md) | `mf2 convert --from leptos-fluent`: the messages and the call sites converted in one command, and what is left to finish by hand |
| [Versioning](versioning.md) | what 1.x promises and what it does not, the Leptos lines, the minimum Rust version (1.88) |

Every `rust`, `toml` and `mf2` block on these pages is part of a small
application, and `cargo xtask docs` compiles each of them, for the targets
it runs on, in CI. A block's info string names its file
(`file=hello/src/lib.rs`). The migration page's `before` blocks are
`leptos-fluent` code: they are not compiled, but the page's commands are run
on them and must produce the files it shows. The reference pages
(`mf2.toml`, Lints, Features) show fragments, not an application: a test
parses their `mf2.toml` samples, and fails when a key, a lint or a feature
has no section. The Testing page's tests are run as well.
