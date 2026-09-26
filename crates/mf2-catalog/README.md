# mf2-catalog

The `.mf2b` binary catalog: one locale's messages and locale data, a
lossless encoding of the MF2 data model that the client reads in place.
The reader is `no_std`, allocation-free and panic-free; the writer, the
model-rebuilding decoder and the manifest file are features, used at
build time.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports the items
they need.

Of its API, 1.x promises the loaded `Catalog` and the error types; the
byte format's views, the writer and the decoder are hidden from the
documentation and may change (`docs/versioning.md`).

API documentation: <https://docs.rs/mf2-catalog>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

Versions: every mf2-two crate is released together, and 1.x keeps the
promise `docs/versioning.md` states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
