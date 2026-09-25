# mf2-macros

The `tr!` procedural macro: a call site checked against the messages'
manifest when it compiles, and lowered to a positional description of
the message.

Applications do not name this crate: the module [`mf2-build`](https://docs.rs/mf2-build) generates
in the translation crate exports the `tr!` an application uses.

API documentation: <https://docs.rs/mf2-macros>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

## License

MIT (`LICENSE`).
