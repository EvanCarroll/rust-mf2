# mf2-host-web

The browser host of the MF2 runtime: Unicode normalization and float
text through the browser's own `String` functions, so the wasm carries
neither; with `datetime-intl`, dates through `Intl.DateTimeFormat`, and
with `intl`, numbers through `Intl.NumberFormat` and `Intl.PluralRules`.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::host_web`, behind its `host-web` feature.

API documentation: <https://docs.rs/mf2-host-web>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

## License

MIT (`LICENSE`).
