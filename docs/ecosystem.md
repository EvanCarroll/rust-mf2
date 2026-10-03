# How the crates fit together

Rust MF2 is a family of crates for using Unicode MessageFormat 2 (MF2) from
Rust, in Leptos web applications and in native command-line and terminal
applications. An application names one of them, `mf2`, and turns on the
integration for its kind as `mf2`'s features: `leptos` and `axum` on the
web, `native` (and `ratatui`, `clap`) natively, and `host-std` or
`host-web` with no framework. None is on by default
([Features of `mf2`](features.md)). Its build script calls `mf2-build`,
which checks the translation resources and writes the generated module the
call sites use and one binary catalog (`.mf2b`) per language; at run time
the runtime formats each call site against the catalog of the active
locale.

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
| [`mf2`](https://docs.rs/mf2) | Application facade: the call-site API and the MF2 runtime, with features for the host and the formatting functions. With a Leptos line and mode, `mf2::leptos`: rendering, reactive arguments, markup, and switching language live. With `axum`, `mf2::axum`: each request's language and the catalogs' routes. With `native`, `mf2::native`: the catalogs embedded or shipped, and the system's language. With `ratatui`, `mf2::ratatui`: messages as Ratatui text, markup as styles. With `clap`, a `--lang` value parser. With no framework, `host-std` or `host-web` alone. |
| [`mf2-build`](https://docs.rs/mf2-build) | Build-time validation and generation of manifests, catalogs, and the Rust module used by call sites. |
| [`mf2-cli`](https://crates.io/crates/mf2-cli) | The `mf2` command for making starters, checking resources, compiling catalogs, and converting or exchanging translations. |

`mf2` is the facade applications use to write and format messages; it is not
just an index of package links. It defines the call-site types (`Tr`,
`TrArgs`, `TrRich`, `TrDyn`, `ArgValue`, `DateTimeValue`, …), and each
integration is a module of it built on those types and the runtime. Without
a Leptos mode, `mf2` compiles no Leptos code: no Leptos, no tachys, no
`reactive_graph`. 1.x's `leptos-mf2`, `mf2-axum`, `mf2-native` and
`mf2-ratatui` are these features now ([Upgrading from 1.x](upgrading.md)).

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
- For a CLI or Ratatui application, turn on `mf2`'s `native` (or
  `ratatui`, which includes it). See
  [Native CLI and Ratatui apps](native-apps.md).
- With no framework (a library, a test, a service of your own), turn on
  `host-std` (or `host-web` in the browser) and the functions your messages
  call. See [Features of `mf2`](features.md).
- To understand the public Rust APIs, follow the docs.rs links in the crate
  map above. The book focuses on concepts and end-to-end use.
