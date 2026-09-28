# mf2-macros

The `tr!` procedural macro: a call site checked against the messages'
manifest when it compiles, and lowered to a positional description of
the message.

Applications do not name this crate: the module [`mf2-build`](https://docs.rs/mf2-build) generates
in the translation crate exports the `tr!` an application uses.

API documentation: <https://docs.rs/mf2-macros>.

The [rust-mf2 book](https://chattyness.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://chattyness.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
