# mf2-build

The MessageFormat 2 build pipeline: from a directory of message
resources, per language, to what an application ships — the manifest,
one binary catalog per locale, and a generated Rust module with the
application's `tr!` macro. It checks every message and reports what a
translation is missing or gets wrong.

A translation crate's `build.rs` runs it:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    mf2_build::Build::new()?.emit_cargo(true).run()?.into_result()?;
    Ok(())
}
```

The `mf2` command ([`mf2-cli`](https://docs.rs/mf2-cli)) creates such a crate (`mf2 init`) and runs
the same checks from the command line.

API documentation: <https://docs.rs/mf2-build>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

Versions: every mf2-two crate is released together, and 1.x keeps the
promise `docs/versioning.md` states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
