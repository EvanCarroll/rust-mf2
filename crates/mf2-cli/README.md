# mf2-cli

The `mf2` command for a MessageFormat 2 corpus: `init` (a starter
application, or translations for a crate), `check`, `compile`, `fmt`,
`stats`, `dump`, `pseudo`, `watch`, `export` and `import` (XLIFF 2), and
`convert` (from Fluent and from `leptos-fluent`).

```sh
cargo install mf2-cli
mf2 --help
```

Every command, its flags and its report codes: the
[command-line chapter](https://evancarroll.github.io/rust-mf2/command-line.html)
of the [Rust MF2 book](https://evancarroll.github.io/rust-mf2/).

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
