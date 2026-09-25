# mf2-fn-number

The localized numeric functions: locale symbols, grouping and numbering
systems over the runtime's numeric core, and `:percent`, `:currency` and
`:unit`. `no_std`, formatting-machinery-free and panic-free.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::fn_number`, behind its `fn-number` feature.

API documentation: <https://docs.rs/mf2-fn-number>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

Versions: every mf2-two crate is released together, and 1.x keeps the
promise `docs/versioning.md` states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
