# mf2-model

The Unicode MessageFormat 2 interchange data model as Rust types, and the
identities (`MsgId`, `Dir`), error kinds and `Frontend` trait the rust-mf2
crates share. `no_std`; with `serde`, JSON in the data model's
interchange shape.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports the items
they need.

API documentation: <https://docs.rs/mf2-model>.

The [rust-mf2 book](https://chattyness.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://chattyness.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
