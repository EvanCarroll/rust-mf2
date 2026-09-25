# mf2-two documentation

Unicode MessageFormat 2 for [Leptos](https://leptos.dev). Translations are
written in MF2 and checked when the application compiles. The browser
downloads one small binary catalog per language, when it needs it, and a
language switch is live. The wasm contains none of the text.

| Page | What it covers |
|---|---|
| [Getting started](getting-started.md) | installing, `mf2 init`, the messages, a server-rendered application that hydrates and switches language live |
| [Call sites](call-sites.md) | `tr!` in text, attributes, props and strings; arguments, signals, dates; markup as elements; plain and isolated text |
| [Delivery modes](delivery-modes.md) | SSR + hydrate, lazy routes, islands, client-only |
| [Switching language](switching.md) | how the server chooses, the switcher, what a switch does, your own control |
| [Accessibility](accessibility.md) | what the library does for WCAG 2.2 AA, and what the application does |
| [Migrating from `leptos-fluent`](migrating-from-leptos-fluent.md) | `mf2 convert --from leptos-fluent`: the messages and the call sites converted in one command, and what is left to finish by hand |

Every `rust`, `toml` and `mf2` block on these pages is part of a small
application, and `cargo xtask docs` compiles each of them, for the server
and for the browser, in CI. A block's info string names its file
(`file=hello/src/lib.rs`). The migration page's `before` blocks are
`leptos-fluent` code: they are not compiled, but the page's commands are run
on them and must produce the files it shows.
