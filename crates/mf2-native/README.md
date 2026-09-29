# mf2-native

1.x's native application support of Rust MF2, kept as a shim. Everything it
named — `NativeI18n`, which holds one generated corpus's catalogs and the
active locale, picks the reader's language from the system's and formats in
the system's time zone; `NativeError`; `LocaleSource` — now lives in
[`mf2`](https://docs.rs/mf2) as `mf2::native`, behind `mf2`'s `native`
feature (`NativeError` is `mf2::native::Error` there, and `NativeI18n` is
kept for this shim beside 2.0's store and `Catalogs`). This crate
re-exports all of it under the names and paths 1.x used, so a 1.x
application keeps compiling, in a workspace beside a browser client too:
`mf2` refuses `native` beside `hydrate` or `csr` only when compiling for
the browser (`wasm32`).

A new application names `mf2` alone:

```toml
[dependencies]
mf2 = { version = "2", features = ["native"] }
```

See the [native application guide](https://evancarroll.github.io/rust-mf2/native-apps.html).

API documentation: <https://docs.rs/mf2-native>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and the book's
[versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html)
states what each release promises; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
