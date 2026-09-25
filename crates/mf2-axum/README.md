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

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

## License

MIT (`LICENSE`).
