# How the crates fit together

Rust MF2 is a family of crates for using Unicode MessageFormat 2 (MF2) from
Rust, in Leptos web applications and in native command-line and terminal
applications. Every application starts with the `mf2` facade, then adds the
integration for its kind: its `leptos` and `axum` features on the web,
`mf2-native` (and `mf2-ratatui`) natively. Translation resources are checked and
compiled by `mf2-build`, which writes the generated module the call sites
use and one binary catalog (`.mf2b`) per language; at run time the runtime
formats each call site against the catalog of the active locale.

```text
build time:  .mf2 resources ──> mf2-build ──> generated module (tr!, ids)
                                          └─> one .mf2b catalog per language

run time:    application ──> call sites (tr!) ──> runtime ──> active catalog
```

The book explains how to use the system. Each crate's rustdoc remains the
reference for its exact API and feature flags.

## Application-facing crates

| Crate | Role |
|---|---|
| [`mf2`](https://docs.rs/mf2) | Application facade: re-exports the MF2 runtime and call-site API, carries feature flags for the host and formatting functions, and provides `Corpus` and `Message` for native applications. With `leptos` (or `leptos-0-8`), `mf2::leptos`: Leptos rendering, reactive arguments, markup rendering, and locale switching. With `axum`, `mf2::axum`: locale negotiation and the catalogs' routes. |
| [`mf2-cli`](https://crates.io/crates/mf2-cli) | The `mf2` command for making starters, checking resources, compiling catalogs, and converting or exchanging translations. |
| [`mf2-build`](https://docs.rs/mf2-build) | Build-time validation and generation of manifests, catalogs, and the Rust module used by call sites. |
| [`mf2-native`](https://docs.rs/mf2-native) | Native CLI and terminal apps: loads one generated corpus (embedded or from files), picks the system's language, keeps the locale in app-owned state. |
| [`mf2-ratatui`](https://docs.rs/mf2-ratatui) | Optional: messages as Ratatui `Text` and `Line`, with MF2 markup as styles. |

`mf2` is the facade applications use to write and format messages; it is not
just an index of package links. It defines the call-site types (`Tr`,
`TrArgs`, `TrRich`, `TrDyn`, `ArgValue`, `DateTimeValue`, …). The native
crates build on those types and on the runtime; `mf2::leptos` adds
rendering and a live locale switch for server-rendered and browser
applications. Without its `leptos` feature, `mf2` compiles no Leptos code:
no Leptos, no tachys, no `reactive_graph`.

## Supporting crates

These crates provide the implementation layers used by the application-facing
crates. Most application code does not need to depend on them directly.

| Crate | Role |
|---|---|
| [`mf2-model`](https://docs.rs/mf2-model) | MF2 message data model and shared identifiers. |
| [`mf2-syntax`](https://docs.rs/mf2-syntax) | Parsing, validation, analysis, and serialization of MF2 messages. |
| [`mf2-resource`](https://docs.rs/mf2-resource) | Reading and writing message resource files. |
| [`mf2-catalog`](https://docs.rs/mf2-catalog) | Binary catalog format, reader, and writer. |
| [`mf2-runtime`](https://docs.rs/mf2-runtime) | Message evaluation, formatting, and output sinks. |
| [`mf2-locale-data`](https://docs.rs/mf2-locale-data) | Locale direction, plural rules, and compact locale data used during builds. |
| [`mf2-fn-number`](https://docs.rs/mf2-fn-number) | Localized number, currency, percent, and unit functions. |
| [`mf2-fn-datetime`](https://docs.rs/mf2-fn-datetime) | Date and time functions with the selected backend. |
| [`mf2-host-std`](https://docs.rs/mf2-host-std) | Native host services for formatting. |
| [`mf2-host-web`](https://docs.rs/mf2-host-web) | Browser host services for formatting. |
| [`mf2-macros`](https://docs.rs/mf2-macros) | Procedural macros used by generated translation modules and `mf2`. |

## Choose a path

- For a Leptos application, start with [Getting started](getting-started.md),
  then see [delivery modes](delivery-modes.md) and
  [switching language](switching.md).
- For a CLI or Ratatui application, use [`mf2-native`](https://docs.rs/mf2-native)
  for catalog and locale state, then add [`mf2-ratatui`](https://docs.rs/mf2-ratatui)
  only if the application uses Ratatui. See
  [Native CLI and Ratatui apps](native-apps.md).
- To understand the public Rust APIs, follow the docs.rs links in the crate
  map above. The book focuses on concepts and end-to-end use.
