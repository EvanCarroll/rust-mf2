# mf2-leptos-ui-0-8

The built-in Leptos 0.8 components of Rust MF2: the locale switcher and
its options, the catalog preload and links, the `hreflang` block and the
islands gate.

Applications do not name this crate: `mf2::leptos` wraps each of its
components, with `mf2`'s `Layer` chosen, when `mf2`'s `leptos-0-8`
feature (Leptos 0.8) is on. It exists because Leptos's `view!` and
`#[component]` write `::leptos` into the crate that uses them, and `mf2`
reaches its two Leptos lines under names of its own; here the line is
simply `leptos`. `mf2-leptos-ui-0-9` and `mf2-leptos-ui-0-8` compile one
source, each against its own line.

API documentation: <https://docs.rs/mf2-leptos-ui-0-8>; the components as
applications use them: <https://docs.rs/mf2>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together; the minimum Rust
version is 1.88.

## License

MIT (`LICENSE`).
