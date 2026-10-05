<!-- Written by `cargo xtask feature-costs`, and held to a fresh measurement every night: do not edit by hand. -->

| Feature | Where | Set against | Adds | Measured on |
|---|---|---|---:|---|
| `leptos-client-number-builtin` | browser | `leptos-client-number-plain` | 2,514 B gzip | the reference workload, 150 messages with `:number` |
| `leptos-client-number-intl` | browser | `leptos-client-number-plain` | -545 B gzip | the reference workload, 150 messages with `:number` |
| `leptos-client-datetime-iso` | browser | `leptos-client-number-builtin` | 5,336 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `leptos-client-datetime-icu` | browser | `leptos-client-number-builtin`, `leptos-client-datetime-iso` | 59,292 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `leptos-client-datetime-icu-cached` | browser | `leptos-client-number-builtin`, `leptos-client-datetime-icu` | 2,231 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `leptos-client-datetime-intl` | browser | `leptos-client-number-builtin`, `leptos-client-datetime-iso` | 247 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `leptos-client-datetime-intl` | browser | `leptos-client-number-builtin` | -7 B gzip | the reference workload, 150 messages with `:number` and plain placeholders, no date: the feature is on and nothing shows a date |
| `native-number-builtin` | native | `native`, `native-number-plain` | 9,920 B stripped | a plain message, a plain placeholder and a plural |
| `native-datetime-iso` | native | `native`, `native-number-plain` | 167,192 B stripped | a plain message, a plain placeholder, a plural and a date in a named zone |
| `native-datetime-icu` | native | `native`, `native-number-plain` | 328,664 B stripped | a plain message, a plain placeholder, a plural and a date in a named zone |
| `native-datetime-icu` | native | `native`, `native-number-plain` | -40 B stripped | a plain message, a plain placeholder and a plural, no date: the feature is on and nothing shows a date |
| `tzdb-bundled` | native | `native`, `native-number-plain`, `native-datetime-iso` | 247,608 B stripped | a plain message, a plain placeholder, a plural and a date in a named zone |
| `tzdb-bundled` | native | `native`, `native-number-plain` | 0 B stripped | no date in any message, so the feature is on and unused |
| `compile` | native | `native`, `native-number-plain` | 8,352,720 B stripped | the canary's messages, and one compiled at run time |
| `ratatui` | native | `native`, `native-number-plain`, `tui` | -14,936 B stripped | a message drawn as a Ratatui `Line` |
| `clap` | native | `native`, `native-number-plain`, `cli` | 8,864 B stripped | `--lang` parsed by clap |

**The date slice, per language.** The brotli bytes the date data adds to each language's catalog, which a browser downloads: the reference workload with `:datetime` messages, built for `hydrate` with each browser formatter, less the same with `leptos-client-datetime-iso`. With `intl` the browser formats dates itself and no date data is downloaded, so that column should be 0.

| Language | With `leptos-client-datetime-icu` | With `leptos-client-datetime-intl` |
|---|---:|---:|
| `ar-XB` | 316 B brotli | 0 B brotli |
| `en` | 466 B brotli | 0 B brotli |
| `en-XA` | 298 B brotli | 0 B brotli |
| `pl` | 317 B brotli | 0 B brotli |

A figure is the size with the feature less the size without it. In the browser: the client wasm of the reference workload at 1,860 call sites, built for `hydrate`, through `wasm-bindgen` and `wasm-opt -Oz`, then `gzip -9`. Native: the smallest native MF2 application (`tools/native-canary`), in release with fat LTO, stripped. `static-locale` and `mark-fallback-lang` change the Leptos layer, which the reference workload does not have, so they are not measured here.

Measured on 2026-10-05 by `cargo xtask feature-costs`, with `rustc 1.98.1`.
