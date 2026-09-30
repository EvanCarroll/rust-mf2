# mf2-build

The MessageFormat 2 build pipeline: from a directory of message
resources, per language, to what an application ships — the manifest,
one binary catalog per locale, and a generated Rust module with the
application's `tr!` macro. It checks every message and reports what a
translation is missing or gets wrong.

The build script of the crate that includes the generated module — the
application, or a translation crate several share — runs it, and reads the
features of that crate's `mf2` dependency through `links`:

```rust
fn main() {
    mf2_build::run();
}
```

The `mf2` command ([`mf2-cli`](https://docs.rs/mf2-cli)) creates such a crate (`mf2 init`) and runs
the same checks from the command line.

API documentation: <https://docs.rs/mf2-build>.

The [Rust MF2 book](https://evancarroll.github.io/rust-mf2/) is the user guide.

Versions: every Rust MF2 crate is released together, and 1.x keeps the
promise [the book's versioning chapter](https://evancarroll.github.io/rust-mf2/versioning.html) states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
