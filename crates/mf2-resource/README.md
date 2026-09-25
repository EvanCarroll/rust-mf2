# mf2-resource

The W3C Message Resource container for MessageFormat 2 — a file of
messages with comments, sections and metadata: a parser, a serializer
and the data model, generic over the message type. `no_std`. The format
is a W3C draft.

Used by [`mf2-build`](https://docs.rs/mf2-build) and [`mf2-cli`](https://docs.rs/mf2-cli) to read and write
`.mf2` files; an application does not name it.

API documentation: <https://docs.rs/mf2-resource>.

The user guide — getting started, call sites, delivery modes, switching
language, accessibility and migrating from `leptos-fluent` — is in the
`docs/` directory of the mf2-two repository.

## License

MIT (`LICENSE`).
