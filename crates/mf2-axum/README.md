# mf2-axum

Locale negotiation and catalog serving for an
[Axum](https://docs.rs/axum) + [Leptos](https://leptos.dev) server: an
ordered list of typed locale sources and sinks (cookie,
`Accept-Language`, path prefix, query parameter), `Content-Language` and
`Vary`, and `/i18n/*` served immutable from the catalogs embedded in the
server binary.

Leptos 0.9 is the default line; for 0.8, turn default features off and
`leptos-0-8` on, as for `leptos-mf2`.

API documentation: <https://docs.rs/mf2-axum>.

The [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
