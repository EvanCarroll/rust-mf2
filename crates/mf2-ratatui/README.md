# mf2-ratatui

1.x's Ratatui support of Rust MF2, kept as a shim. Everything it named —
`text` and `line`, which format a call site into owned Ratatui `Text` and
`Line` values with the message's markup as styles, and `MarkupStyles`, the
style of each markup name — now lives in [`mf2`](https://docs.rs/mf2) as
`mf2::ratatui`, behind `mf2`'s `ratatui` feature, which implies `native`.
This crate re-exports all of it under the names and paths 1.x used, so a
1.x application keeps compiling, in a workspace beside a browser client
too: `mf2` refuses `ratatui` beside `hydrate` or `csr` only when compiling
for the browser (`wasm32`).

A new application names `mf2` alone:

```toml
[dependencies]
mf2 = { version = "2", features = ["ratatui"] }
```

See the [native application guide](https://evancarroll.github.io/rust-mf2/native-apps.html).

API documentation: <https://docs.rs/mf2-ratatui>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and the book's
[versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html)
states what each release promises; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
