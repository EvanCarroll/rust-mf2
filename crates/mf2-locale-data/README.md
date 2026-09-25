# mf2-locale-data

The CLDR locale data that mf2-two catalogs carry, for the build side
only (never linked into a client): plural rules, text direction, number
symbols and patterns, currencies and units for every CLDR locale, and
the per-locale entries built from them.

Used by [`mf2-build`](https://docs.rs/mf2-build) and by `mf2`'s `compile` feature; an application
does not name it.

API documentation: <https://docs.rs/mf2-locale-data>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

## License

MIT (`LICENSE`) for the code. The tables in `data/` are derived from
the Unicode CLDR and are under the Unicode License v3
(`LICENSE-UNICODE`); the package's licence is `MIT AND Unicode-3.0`.
