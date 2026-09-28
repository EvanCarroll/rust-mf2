# mf2-host-std

The native host of the MF2 runtime, for servers, tests and
`wasm32-wasip1`: Unicode normalization through `unicode-normalization`,
float text through `ryu`, and time-zone offsets from `jiff`'s bundled
IANA database, so every server answers alike.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::host_std`, behind its `host-std` feature.

API documentation: <https://docs.rs/mf2-host-std>.

The [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
