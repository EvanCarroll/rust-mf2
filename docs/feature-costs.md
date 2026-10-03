<!-- Written by `cargo xtask feature-costs`, and held to a fresh measurement every night: do not edit by hand. -->

| Feature | Where | Set against | Adds | Measured on |
|---|---|---|---:|---|
| `fn-number` | browser | no function | 2,485 B gzip | the reference workload, 150 messages with `:number` |
| `number-intl` | browser | `fn-number` | -3,057 B gzip | the reference workload, 150 messages with `:number` |
| `fn-datetime` | browser | `fn-number` | 5,692 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `datetime-icu` | browser | `fn-number`, `fn-datetime` | 99,887 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `datetime-intl` | browser | `fn-number`, `fn-datetime` | 239 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `fn-number` | native | `native` | 8,080 B stripped | a plain message and a plural |
| `fn-datetime` | native | `native` | 168,360 B stripped | a plain message, a plural and a date in a named zone |
| `tzdb-bundled` | native | `native`, `fn-datetime` | 247,624 B stripped | a plain message, a plural and a date in a named zone |
| `tzdb-bundled` | native | `native` | 0 B stripped | no date in any message, so the feature is on and unused |
| `compile` | native | `native` | 8,354,736 B stripped | the canary's messages, and one compiled at run time |
| `ratatui` | native | `native`, `tui` | -14,936 B stripped | a message drawn as a Ratatui `Line` |
| `clap` | native | `native`, `cli` | 8,576 B stripped | `--lang` parsed by clap |

A figure is the size with the feature less the size without it. In the browser: the client wasm of the reference workload at 1,860 call sites, built for `hydrate`, through `wasm-bindgen` and `wasm-opt -Oz`, then `gzip -9`. Native: the smallest native MF2 application (`tools/native-canary`), in release with fat LTO, stripped. `static-locale` and `mark-fallback-lang` change the Leptos layer, which the reference workload does not have, so they are not measured here.

Measured on 2026-10-03 by `cargo xtask feature-costs`, with `rustc 1.98.1`.
