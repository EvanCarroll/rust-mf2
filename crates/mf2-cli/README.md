# mf2-cli

The `mf2` command for a MessageFormat 2 corpus: `init` (a translation
crate), `check`, `compile`, `fmt`, `stats`, `dump`, `pseudo`, `watch`,
`export` and `import` (XLIFF 2), and `convert` (from Fluent and from
`leptos-fluent`).

```sh
cargo install mf2-cli
mf2 --help
```

API documentation: <https://docs.rs/mf2-cli>.

The [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
