# mf2-fn-number

The localized numeric functions: locale symbols, grouping and numbering
systems over the runtime's numeric core, and `:percent`, `:currency` and
`:unit`. `no_std`, formatting-machinery-free and panic-free.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::fn_number`, behind its `fn-number` feature.

API documentation: <https://docs.rs/mf2-fn-number>.

The [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
