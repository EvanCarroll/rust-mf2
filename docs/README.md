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
| [Getting started](getting-started.md) | installing, `mf2 init`, the messages, a server-rendered application that hydrates and switches language live |
| [Call sites](call-sites.md) | `tr!` in text, attributes, props and strings; arguments, signals, dates; markup as elements; plain and isolated text |
| [Delivery modes](delivery-modes.md) | SSR + hydrate, lazy routes, islands, client-only |
| [Switching language](switching.md) | how the server chooses, the switcher, what a switch does, your own control |
| [Native CLI and Ratatui apps](native-apps.md) | `mf2-native` for a command-line or terminal application: embedded or shipped catalogs, the system's language, Ratatui text with markup as styles |
| [The command line](command-line.md) | every `mf2` command: `check`, `fmt`, `compile`, `stats`, `dump`, `export` and `import` (JSON, XLIFF 2), `pseudo`, `watch`, `convert --from fluent` and its report codes |
| [Accessibility](accessibility.md) | what the library does for WCAG 2.2 AA, and what the application does |
| [Migrating from `leptos-fluent`](migrating-from-leptos-fluent.md) | `mf2 convert --from leptos-fluent`: the messages and the call sites converted in one command, and what is left to finish by hand |
| [Versioning](versioning.md) | what 1.x promises and what it does not, the Leptos lines, the minimum Rust version (1.88) |

Every `rust`, `toml` and `mf2` block on these pages is part of a small
application, and `cargo xtask docs` compiles each of them, for the targets
it runs on, in CI. A block's info string names its file
(`file=hello/src/lib.rs`). The migration page's `before` blocks are
`leptos-fluent` code: they are not compiled, but the page's commands are run
on them and must produce the files it shows.
