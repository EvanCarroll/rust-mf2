# mf2-resource

The W3C Message Resource container for MessageFormat 2 — a file of
messages with comments, sections and metadata: a parser, a serializer
and the data model, generic over the message type. `no_std`. The format
is a W3C draft.

Used by [`mf2-build`](https://docs.rs/mf2-build) and [`mf2-cli`](https://docs.rs/mf2-cli) to read and write
`.mf2` files; an application does not name it. Its Rust API mirrors the
draft and follows it, so 1.x does not promise it: what is promised is
the file format as `mf2 fmt` writes it (`docs/versioning.md`).

API documentation: <https://docs.rs/mf2-resource>.

The [rust-mf2 book](https://chattyness.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://chattyness.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
