# mf2-locale-data

The CLDR locale data that Rust MF2 catalogs carry, for the build side
only (never linked into a client): plural rules, text direction, number
symbols and patterns, currencies and units for every CLDR locale, and
the per-locale entries built from them.

Used by [`mf2-build`](https://docs.rs/mf2-build) and by `mf2`'s `compile` feature; an application
does not name it.

Of its API, 2.x promises the errors, `CLDR_VERSION` and `direction`; the
tables and entry builders are hidden from the documentation and may
change (`docs/versioning.md`).

API documentation: <https://docs.rs/mf2-locale-data>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`) for the code. The tables in `data/` are derived from
the Unicode CLDR and are under the Unicode License v3
(`LICENSE-UNICODE`); the package's licence is `MIT AND Unicode-3.0`.
