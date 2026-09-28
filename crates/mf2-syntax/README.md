# mf2-syntax

MessageFormat 2 syntax: a parser to a lossless concrete syntax tree
with error recovery, lowering to the data model, the Data Model Errors,
a serializer back to MF2 source, and the variable analysis a build's
manifest needs. `no_std`.

Used by [`mf2-build`](https://docs.rs/mf2-build), [`mf2-cli`](https://docs.rs/mf2-cli) and `mf2`'s `compile` feature;
an application does not name it.

API documentation: <https://docs.rs/mf2-syntax>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
