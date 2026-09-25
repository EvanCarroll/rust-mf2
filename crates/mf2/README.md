# mf2

Unicode MessageFormat 2 for [Leptos](https://leptos.dev): the one crate an
application's code names. It re-exports the public API of the mf2-two
crates and carries the feature flags that choose what a build includes:
localized numbers (`fn-number`), dates (`fn-datetime` and a backend),
the host (`host-std` on a server, `host-web` in the browser), the Leptos
target (`ssr`, `hydrate` or `csr`), and `compile_str` for an ad-hoc
message.

Messages are written in MF2, checked when the application compiles, and
delivered as one small binary catalog per language, loaded when it is
needed. The client wasm contains none of the text.

An application's translation crate depends on `mf2` and, as a build
dependency, [`mf2-build`](https://docs.rs/mf2-build); the `mf2` command ([`mf2-cli`](https://docs.rs/mf2-cli)) creates it.

API documentation: <https://docs.rs/mf2>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

Versions: every mf2-two crate is released together, and 1.x keeps the
promise `docs/versioning.md` states; the minimum Rust version is 1.88.

## License

MIT (`LICENSE`).
