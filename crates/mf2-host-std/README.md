# mf2-host-std

The native host of the MF2 runtime, for servers, tests and
`wasm32-wasip1`: float text through `core`, and time-zone offsets from
`jiff`'s bundled IANA database, so every server answers alike.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::host_std`, behind its `host-std` feature.

API documentation: <https://docs.rs/mf2-host-std>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
