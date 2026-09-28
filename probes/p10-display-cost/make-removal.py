#!/usr/bin/env python3
"""A9: write the removal variants of A5's `leptos-mf2` into the tree, one at
a time, so that `git diff` saves each as a patch (lib-r*.patch).

    make-removal.py r1|r1d|r2|r2d     (from the root of a clean A5 tree)

    r1   `Display` only with the std mode (`ssr`; 2.0 adds `native`): A5's
         fallback
    r2   no `Display` on wasm32-unknown-unknown, whatever the features
    r1d, r2d   the same, and every `Debug` A5 added too; `Tr` keeps the
         `Debug` it derived in 1.x
"""
import pathlib
import sys

variant = sys.argv[1]
PRED = {
    "r1": 'feature = "ssr"',
    "r2": 'not(all(target_arch = "wasm32", target_os = "unknown"))',
}[variant[:2]]
debug_too = variant.endswith("d")
src = pathlib.Path("crates/leptos-mf2/src")


def sub(path, old, new, count=1):
    p = src / path
    t = p.read_text()
    if t.count(old) != count:
        sys.exit(f"{path}: {t.count(old)} matches for {old[:70]!r}")
    p.write_text(t.replace(old, new))


# Display: the impls, and what only they call.
sub("display.rs",
    "        /// The text [`to_string`](Self::to_string) returns, written into the\n"
    "        /// formatter — padded, when the format asks (`{:<12}`).\n"
    "        impl fmt::Display for $ty {",
    "        /// The text [`to_string`](Self::to_string) returns, written into the\n"
    "        /// formatter — padded, when the format asks (`{:<12}`).\n"
    f"        #[cfg({PRED})]\n"
    "        impl fmt::Display for $ty {")
sub("display.rs",
    "    pub(super) fn fmt<D: Description>(description: &D, f: &mut fmt::Formatter<'_>) -> fmt::Result {",
    f"    #[cfg({PRED})]\n"
    "    pub(super) fn fmt<D: Description>(description: &D, f: &mut fmt::Formatter<'_>) -> fmt::Result {")
sub("display.rs",
    "    pub(super) fn fmt<D>(_description: &D, _f: &mut fmt::Formatter<'_>) -> fmt::Result {",
    f"    #[cfg({PRED})]\n"
    "    pub(super) fn fmt<D>(_description: &D, _f: &mut fmt::Formatter<'_>) -> fmt::Result {")
sub("text.rs",
    "pub(crate) fn fmt_display<D: Description>(",
    f"#[cfg({PRED})]\npub(crate) fn fmt_display<D: Description>(")

if debug_too:
    derive = f"#[cfg_attr({PRED}, derive(Debug))]\n"
    impl = f"#[cfg({PRED})]\n"
    # derives A5 added (Tr's is 1.x's and stays)
    sub("tr.rs", "#[derive(Clone, Debug)]\npub struct TrArgs {", "#[derive(Clone)]\n" + derive + "pub struct TrArgs {")
    sub("tr.rs", "impl core::fmt::Debug for TrRich {", impl + "impl core::fmt::Debug for TrRich {")
    sub("dynamic.rs", "#[derive(Clone, Debug)]\npub struct TrDyn {", "#[derive(Clone)]\n" + derive + "pub struct TrDyn {")
    sub("arg.rs", "impl core::fmt::Debug for Text {", impl + "impl core::fmt::Debug for Text {")
    sub("arg.rs", "#[derive(Clone, Debug)]\npub struct DateTimeValue {", "#[derive(Clone)]\n" + derive + "pub struct DateTimeValue {")
    sub("arg.rs", "impl core::fmt::Debug for ArgValue {", impl + "impl core::fmt::Debug for ArgValue {")
    sub("arg.rs", "impl core::fmt::Debug for ArgList {", impl + "impl core::fmt::Debug for ArgList {")
    sub("markup.rs", "#[derive(Debug)]\npub struct Handler<H>(pub H);", derive + "pub struct Handler<H>(pub H);")
    sub("markup.rs", "    impl core::fmt::Debug for NestingHandler {", "    " + impl + "    impl core::fmt::Debug for NestingHandler {")
    sub("markup.rs", "    #[derive(Debug)]\n    pub struct Flat<F>(pub F);", "    " + derive + "    pub struct Flat<F>(pub F);")
    sub("markup.rs", "    impl core::fmt::Debug for FlatHandler {", "    " + impl + "    impl core::fmt::Debug for FlatHandler {")
    sub("text.rs", "#[derive(Clone, Debug)]\n#[non_exhaustive]\npub enum Stored {", "#[derive(Clone)]\n" + derive + "#[non_exhaustive]\npub enum Stored {")
    for path in ("catalog.rs", "signal.rs"):
        p = src / path
        t = p.read_text()
        n = t.count("#[derive(Clone, Debug)]\n") + t.count("#[derive(Debug)]\n")
        t = t.replace("#[derive(Clone, Debug)]\n", "#[derive(Clone)]\n" + derive)
        t = t.replace("#[derive(Debug)]\n", derive)
        p.write_text(t)
        print(f"{path}: {n} derives gated")
print(f"{variant}: written")
