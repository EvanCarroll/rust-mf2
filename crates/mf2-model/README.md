# mf2-model

The Unicode MessageFormat 2 interchange data model as Rust types, and the
identities (`MsgId`, `Dir`), error kinds and `Frontend` trait the rust-mf2
crates share. `no_std`; with `serde`, JSON in the data model's
interchange shape.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports the items
they need.

API documentation: <https://docs.rs/mf2-model>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

Versions: every mf2-two crate is released together, and 1.x keeps the
promise `docs/versioning.md` states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
