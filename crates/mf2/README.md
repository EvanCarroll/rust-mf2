# mf2

Unicode MessageFormat 2 for Rust applications — web applications with
[Leptos](https://leptos.dev), and native command-line and terminal
applications. `mf2` is the one crate an application's code names. It
re-exports the public API of the Rust MF2 crates and carries the feature
flags that choose what a build includes: localized numbers
(`fn-number`), dates (`fn-datetime` and a backend), the host
(`host-std` natively, `host-web` in the browser), the Leptos target
(`ssr`, `hydrate` or `csr`), and `compile_str` for an ad-hoc message.

Messages are written in MF2, checked when the application compiles, and
compiled to one small binary catalog per language. On the web, the
browser loads a language's catalog when it needs it, and the client wasm
contains none of the text.

An application's translation crate depends on `mf2` and, as a build
dependency, [`mf2-build`](https://docs.rs/mf2-build). Beside them, a web
application adds [`leptos-mf2`](https://docs.rs/leptos-mf2) and
[`mf2-axum`](https://docs.rs/mf2-axum); a native one adds
[`mf2-native`](https://docs.rs/mf2-native) and, for a terminal UI,
[`mf2-ratatui`](https://docs.rs/mf2-ratatui). For native applications
`mf2` also provides `Corpus`, the one value a native build generates, and
`Message`, which formats any `tr!` call site outside Leptos.

API documentation: <https://docs.rs/mf2>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
