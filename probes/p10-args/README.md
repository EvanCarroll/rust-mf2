# p10-args — Phase 10 A8: how `tr!` converts an argument

A probe for the design (`plans/19-native-and-terminal.md` §7), deleted at
Phase 10's exit; its result is recorded there. A standalone crate with no
dependencies: `src/lib.rs` stands in for `mf2`'s `ArgValue` and a new
`IntoArg` trait, and `arg!(e)` for what `tr!` would emit for one argument.

The question: can a call site pass any type that has a text (`Display`) —
trippy's `KeyBinding`, an `io::Error`, an `Ipv4Addr` — while numbers, strings,
dates and paths keep their typed conversion, and a type with neither still
gets a message of ours rather than one about `ArgValue`?

The expansion is a three-step method dispatch on a wrapper, resolved where
the argument's type is concrete:

1. `Wrap<T>` by value, when `T: IntoArg` — the typed conversion;
2. `&Wrap<T>`, when `T: Display` — the value's text;
3. `&mut Wrap<T>`, always; its method requires `T: IntoArg`, so a type with
   neither gets `IntoArg`'s `#[diagnostic::on_unimplemented]` message.

```sh
./run.sh     # results/accepted.txt, results/refused-*.txt
```

`examples/accepted.rs` checks which step each type takes (13 cases, among
them a type with both, and generic code bounded on `Display` or on both);
`examples/refused.rs` has one call per feature that must not compile: a type
with neither, a generic `T` with no bound, and an `Option`.
