# leptos-mf2

1.x's Leptos layer of Rust MF2, kept as a shim. Everything it named — the
call-site types (`Tr`, `TrArgs`, `TrRich`, `ArgValue`), the rendering, the
per-locale catalog, the live switch, the components, `<html lang dir>` and
the reader's time zone — now lives in [`mf2`](https://docs.rs/mf2): the
Leptos layer as `mf2::leptos`, the call-site types at `mf2`'s root. This
crate re-exports all of it under the paths 1.x used, and forwards its
features to `mf2`'s, so a 1.x application keeps compiling, beside a native
application on `mf2-native` in one workspace too: `mf2` refuses `native`
beside `hydrate` or `csr` only when compiling for the browser (`wasm32`).

A new application names `mf2` alone, with its Leptos line:

```toml
[dependencies]
mf2 = { version = "2", features = ["leptos"] }   # Leptos 0.9; `leptos-0-8` for 0.8

[features]
ssr = ["leptos/ssr", "mf2/ssr"]
hydrate = ["leptos/hydrate", "mf2/hydrate"]
```

Here, Leptos 0.9 is the default line (`leptos-0-9`); for 0.8, turn default
features off and `leptos-0-8` on.

API documentation: <https://docs.rs/leptos-mf2>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and the book's
[versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html)
states what each release promises; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
