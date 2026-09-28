# mf2-host-web

The browser host of the MF2 runtime: Unicode normalization and float
text through the browser's own `String` functions, so the wasm carries
neither; with `datetime-intl`, dates through `Intl.DateTimeFormat`, and
with `intl`, numbers through `Intl.NumberFormat` and `Intl.PluralRules`.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::host_web`, behind its `host-web` feature.

API documentation: <https://docs.rs/mf2-host-web>.

The [rust-mf2 book](https://chattyness.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://chattyness.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
