# mf2-syntax

MessageFormat 2 syntax: a parser to a lossless concrete syntax tree
with error recovery, lowering to the data model, the Data Model Errors,
a serializer back to MF2 source, and the variable analysis a build's
manifest needs. `no_std`.

Used by [`mf2-build`](https://docs.rs/mf2-build), [`mf2-cli`](https://docs.rs/mf2-cli) and `mf2`'s `compile` feature;
an application does not name it.

API documentation: <https://docs.rs/mf2-syntax>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

## License

MIT (`LICENSE`).
