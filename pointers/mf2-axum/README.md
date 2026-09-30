# mf2-axum — moved into `mf2`

Since 2.0, the Axum support of rust-mf2 is part of the
[`mf2`](https://crates.io/crates/mf2) crate, behind its `axum` feature. This
2.0.0 release holds no code: building it stops with a message saying what to
write instead.

```toml
[dependencies]
mf2 = { version = "2", features = ["axum"] }
```

Applications that depend on `mf2-axum = "1"` keep building on 1.0.0.

The upgrade guide: <https://evancarroll.github.io/rust-mf2/upgrading.html>.
