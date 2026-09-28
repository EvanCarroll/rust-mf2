# mf2-runtime

The MessageFormat 2 evaluator: formats a message from a `.mf2b`
catalog — resolution, selection, fallback, bidi isolation, format to
parts, markup — and holds the function registry, the custom-function
API and the core functions. `no_std`, with no formatting machinery and
no panics on the client path.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports the items
they need.

API documentation: <https://docs.rs/mf2-runtime>.

The [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
