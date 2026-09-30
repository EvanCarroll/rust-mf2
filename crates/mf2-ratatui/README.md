# mf2-ratatui

1.x's Ratatui support of Rust MF2, now a pointer: it re-exports
[`mf2`](https://docs.rs/mf2)'s `mf2::ratatui`, where the code lives, behind
`mf2`'s `ratatui` feature, which implies `native`. 1.x's `text`, `line` and
`MarkupStyles`, which took a handle and a map of styles on every call, are
gone in 2.0: a call site converts into Ratatui's `Line` or `Text` itself, and
a `Theme`, set once, says how its markup is drawn.

An application names `mf2` alone:

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
