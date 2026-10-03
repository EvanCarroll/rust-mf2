# Rust MF2

Unicode MessageFormat 2 (MF2) for Rust applications: web applications with
[Leptos](https://leptos.dev), and native command-line and terminal
applications, including Ratatui. You write translations in MF2, the build
checks them, and `tr!("id", name = value)` shows a message in the reader's
language.

## One crate

An application depends on **`mf2`**, and on `mf2-build` for its build
script. `mf2`'s features choose the rest, and none is on by default:

| Feature | For |
|---|---|
| `leptos` (or `leptos-0-8`) with `ssr`, `hydrate` or `csr` | a Leptos application, server-rendered, hydrated or client-only |
| `axum` | the server: each request's language, and the catalogs' routes |
| `native` | a command-line tool, in the system's language |
| `ratatui` | a Ratatui terminal UI, with markup as styles |
| `clap` | a `--lang` option parsed by clap |
| `host-std` or `host-web` | `mf2` with no framework |
| `fn-number`, `fn-datetime` and a date backend | numbers and dates in each language's own way |
| `number-intl`, `datetime-intl`, `tzdb-bundled` | whose locale data: the platform's and a smaller build, or the same answer everywhere |

[Features of `mf2`](features.md) says what each does and what it costs.

The `mf2` command (`cargo install mf2-cli`) makes starters, checks
translations and exchanges them with translators.

> **What happens underneath.** The build turns each language into one small
> binary catalog. A browser downloads a language's catalog when it needs it,
> switches language without a reload, and its wasm carries none of the
> text. A native application embeds its catalogs or ships them beside the
> executable. [How the crates fit together](ecosystem.md) has the rest.

## Where to start

* **A Leptos application:** [Getting started](getting-started.md), then
  [Call sites](call-sites.md).
* **A command-line tool or a terminal UI:**
  [Native CLI and Ratatui apps](native-apps.md).
* **The message language:** [MF2 for developers](mf2-for-developers.md).
* **Coming from `leptos-fluent`:**
  [Migrating from `leptos-fluent`](migrating-from-leptos-fluent.md).
* **Coming from `mf2` 1.x:** [Upgrading from 1.x](upgrading.md).

> **Every sample is compiled.** Each `rust`, `toml` and `mf2` block in this
> book names the file of a small application it belongs to, and
> `cargo xtask docs` builds those applications in CI, for the targets they
> run on. The exceptions are marked: the migration page's `leptos-fluent`
> code, which its commands convert; the upgrade page's 1.x excerpts; and the
> reference pages' fragments, which a test parses instead.
