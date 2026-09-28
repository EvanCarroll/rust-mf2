# mf2-fn-datetime

The MessageFormat 2 date and time functions — `:datetime`, `:date`,
`:time` and unannotated date/time values — with their operand, option
and time-zone semantics, over a choice of backend: a neutral stub,
ICU4X, or the browser's `Intl.DateTimeFormat`. `no_std`.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::fn_datetime`, behind its `fn-datetime` feature (with `datetime-icu`
or `datetime-intl` for a backend).

API documentation: <https://docs.rs/mf2-fn-datetime>.

The [rust-mf2 book](https://evancarroll.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
