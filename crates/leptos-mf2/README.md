# leptos-mf2

The Leptos side of rust-mf2: what a `tr!` call site builds (`Tr`,
`TrArgs`, `TrRich`, `ArgValue`) and, with a Leptos target feature
(`ssr`, `hydrate` or `csr`), how it renders — in text, attributes and
props — together with the per-locale catalog, the live locale switch,
`<html lang dir>`, and the reader's time zone for dates.

Leptos 0.9 is the default line; for 0.8, turn default features off and
`leptos-0-8` on. `static-locale` makes a switch a navigation (for
islands), and `mark-fallback-lang` marks text borrowed from a fallback
language with its own `lang`.

API documentation: <https://docs.rs/leptos-mf2>.

The [rust-mf2 book](https://chattyness.github.io/rust-mf2/) covers the ecosystem
and application guides, including CLI and Ratatui integrations.

Versions: every rust-mf2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://chattyness.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
