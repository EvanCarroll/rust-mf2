# mf2-native

Native CLI and terminal applications for Rust MF2. `NativeI18n` holds the
catalogs of one generated corpus and an active locale the application owns.
It picks the first of the system's preferred languages the corpus supports,
else the source locale. It formats the `tr!` call sites of the application's
i18n crate in the system's time zone, with bidi isolation off; both can be
changed.

```rust,ignore
let mut i18n = mf2_native::NativeI18n::embedded(&my_i18n::CORPUS)?;
if let Some(lang) = args.lang.as_deref() {
    i18n.set_locale(lang)?; // an unsupported language is an error
}
println!("{}", i18n.format(&my_i18n::tr!("welcome")));
```

The i18n crate's build script runs `mf2_build` with `Emit::Native` (catalogs
embedded) or `Emit::NativeFiles` (catalogs shipped beside the executable and
loaded with `NativeI18n::from_directory`). Argument parsing and persistence
stay with the application.

See the [native application guide](https://evancarroll.github.io/rust-mf2/native-apps.html);
[`mf2-ratatui`](https://docs.rs/mf2-ratatui) turns messages into Ratatui text.

API documentation: <https://docs.rs/mf2-native>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
