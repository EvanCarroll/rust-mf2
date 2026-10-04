# mf2-fn-datetime-web-icu

ICU4X for the browser build of Rust MF2's date functions: the crates
`mf2-fn-datetime`'s `web-icu` feature formats with, re-exported. A native
build never compiles them for the browser's sake, and a browser build
compiles them only when it formats dates with ICU4X. `no_std`.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2)'s
browser date features (`host-web-datetime-icu`, `leptos-client-datetime-icu`)
reach it through `mf2-fn-datetime`.

API documentation: <https://docs.rs/mf2-fn-datetime-web-icu>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
