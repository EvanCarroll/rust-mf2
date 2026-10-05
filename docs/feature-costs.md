<!-- Written by `cargo xtask feature-costs`, and held to a fresh measurement every night: do not edit by hand. -->

| Feature | Where | Set against | Adds | Measured on |
|---|---|---|---:|---|
| `fn-number` | browser | no function | 2,494 B gzip | the reference workload, 150 messages with `:number` |
| `number-intl` | browser | `fn-number` | -3,069 B gzip | the reference workload, 150 messages with `:number` |
| `leptos-client-datetime-iso` | browser | `fn-number` | 5,337 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `leptos-client-datetime-icu` | browser | `fn-number`, `leptos-client-datetime-iso` | 59,272 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `leptos-client-datetime-intl` | browser | `fn-number`, `leptos-client-datetime-iso` | 253 B gzip | the reference workload, 150 messages with `:number` and 150 with `:datetime` |
| `leptos-client-datetime-intl` | browser | `fn-number` | -2 B gzip | the reference workload, 150 messages with `:number` and plain placeholders, no date: the feature is on and nothing shows a date |
| `fn-number` | native | `native` | 9,920 B stripped | a plain message, a plain placeholder and a plural |
| `native-datetime-iso` | native | `native` | 167,320 B stripped | a plain message, a plain placeholder, a plural and a date in a named zone |
| `native-datetime-icu` | native | `native` | 328,808 B stripped | a plain message, a plain placeholder, a plural and a date in a named zone |
| `native-datetime-icu` | native | `native` | 146,080 B stripped | a plain message, a plain placeholder and a plural, no date: the feature is on and nothing shows a date |
| `tzdb-bundled` | native | `native`, `native-datetime-iso` | 247,608 B stripped | a plain message, a plain placeholder, a plural and a date in a named zone |
| `tzdb-bundled` | native | `native` | 0 B stripped | no date in any message, so the feature is on and unused |
| `compile` | native | `native` | 8,352,880 B stripped | the canary's messages, and one compiled at run time |
| `ratatui` | native | `native`, `tui` | -14,936 B stripped | a message drawn as a Ratatui `Line` |
| `clap` | native | `native`, `cli` | 8,864 B stripped | `--lang` parsed by clap |

**The date slice, per language.** The brotli bytes the date data adds to each language's catalog, which a browser downloads: the reference workload with `:datetime` messages, built for `hydrate` with each browser formatter, less the same with `leptos-client-datetime-iso`. With `intl` the browser formats dates itself and no date data is downloaded, so that column should be 0.

| Language | With `leptos-client-datetime-icu` | With `leptos-client-datetime-intl` |
|---|---:|---:|
| `ar-XB` | 316 B brotli | 0 B brotli |
| `en` | 466 B brotli | 0 B brotli |
| `en-XA` | 298 B brotli | 0 B brotli |
| `pl` | 317 B brotli | 0 B brotli |

A figure is the size with the feature less the size without it. In the browser: the client wasm of the reference workload at 1,860 call sites, built for `hydrate`, through `wasm-bindgen` and `wasm-opt -Oz`, then `gzip -9`. Native: the smallest native MF2 application (`tools/native-canary`), in release with fat LTO, stripped. `static-locale` and `mark-fallback-lang` change the Leptos layer, which the reference workload does not have, so they are not measured here.

Measured on 2026-10-05 by `cargo xtask feature-costs`, with `rustc 1.98.1`.
