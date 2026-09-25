# mf2-host-std

The native host of the MF2 runtime, for servers, tests and
`wasm32-wasip1`: Unicode normalization through `unicode-normalization`,
float text through `ryu`, and time-zone offsets from `jiff`'s bundled
IANA database, so every server answers alike.

Applications do not name this crate: [`mf2`](https://docs.rs/mf2) re-exports it as
`mf2::host_std`, behind its `host-std` feature.

API documentation: <https://docs.rs/mf2-host-std>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

## License

MIT (`LICENSE`).
