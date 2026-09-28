# mf2-model

The Unicode MessageFormat 2 interchange data model as Rust types, and the
identities (`MsgId`, `Dir`), error kinds and `Frontend` trait the Rust MF2
crates share. `no_std`; with `serde`, JSON in the data model's
interchange shape.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports the items
they need.

API documentation: <https://docs.rs/mf2-model>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
