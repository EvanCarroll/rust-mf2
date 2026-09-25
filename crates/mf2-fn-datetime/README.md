# mf2-fn-datetime

The MessageFormat 2 date and time functions — `:datetime`, `:date`,
`:time` and unannotated date/time values — with their operand, option
and time-zone semantics, over a choice of backend: a neutral stub,
ICU4X, or the browser's `Intl.DateTimeFormat`. `no_std`.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::fn_datetime`, behind its `fn-datetime` feature (with `datetime-icu`
or `datetime-intl` for a backend).

API documentation: <https://docs.rs/mf2-fn-datetime>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

Versions: every mf2-two crate is released together, and 1.x keeps the
promise `docs/versioning.md` states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
