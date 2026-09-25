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

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

Versions: every mf2-two crate is released together, and 1.x keeps the
promise `docs/versioning.md` states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
