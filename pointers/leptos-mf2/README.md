# leptos-mf2 — moved into `mf2`

Since 2.0, the Leptos layer of rust-mf2 is part of the
[`mf2`](https://crates.io/crates/mf2) crate, behind its `leptos` feature
(Leptos 0.9; `leptos-0-8` for Leptos 0.8) and one mode: `ssr`, `hydrate` or
`csr`. This 2.0.0 release holds no code: building it stops with a message
saying what to write instead.

```toml
[dependencies]
mf2 = { version = "2", features = ["leptos", "ssr"] }
```

Applications that depend on `leptos-mf2 = "1"` keep building on 1.0.0.

The upgrade guide: <https://evancarroll.github.io/rust-mf2/upgrading.html>.
