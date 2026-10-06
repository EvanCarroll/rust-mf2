# What runs where

Four workflows, one runner: Forgejo's `docker` label, in
`leptos-builder:latest` — the image the other Rust repositories here build
with. It is Alpine, so **everything native is musl**.

| workflow | when | what |
|---|---|---|
| `ci.yml` | every push and pull request | the twelve gates, then `report` |
| `nightly.yml` | 03:17 daily, or by hand | the long runs, the size gate, the feature sets, then `report` |
| `verify.yml` | by hand, or a `v*` tag | the whole check suite in one job, compared against the last one |
| `rebaseline.yml` | by hand only | take every figure again and hand back what has to change |

`release` in `ci.yml` is gated to a tag or a manual run: it repeats the `ci`,
`msrv`, `docs-rs` and `package` work, so it is not worth a push. It never
publishes — `cargo xtask release --publish` refuses to run where `CI` is set, by
design. Publishing is the owner's, on the owner's machine.

## The report

Every workflow ends in a `report` job that downloads every artifact the run
produced and folds them into one `REPORT.md`, uploaded as artifact `mf2-report`
and printed into the job's log, so a run can be read without downloading
anything. It is headed with the versions of the tools that produced the figures
— `rustc`, `wasm-opt`, `wasm-bindgen`, `brotli`, `gzip`, the host triple — and
it names the checks this runner cannot do, from `tools/ci/local-only.txt`.

The image's digest is not visible to a job, so those versions are what makes a
figure attributable. A number from one image is not comparable with a number
from another.

## What does not run here

`tools/ci/local-only.txt` is the list, with the command for each. The image has
no browser engines (owner, 2026-10-05), so conformance layers L4 in three
engines, L6, L7 and the churn and demo checks are the owner's to run before a
release. This is the one gap in what CI judges, and every report names it.

## Two things the owner sets up on the Forgejo side

1. A runner with the label `docker`, registered for this repository.
2. `REGISTRY_TOKEN` as a repository or organisation secret, with read access to
   the package namespace `leptos-builder` is published in — it lives under a
   different owner than this repository, so the repository's own token is not
   enough.

The runner's capacity matters: the twelve `ci.yml` jobs have no `needs:` between
them and each builds the workspace from scratch, so at capacity twelve that is
twelve multi-gigabyte `target/` directories at once. Two or three is the safer
start. sccache (the Forgejo runner's own cache, picked up automatically) and the
cargo-registry cache take the repeated work out; they change no measured byte,
because the compiler, the flags and the output are the same.

## Two things to know before reading a figure

**The unit is brotli.** Every size figure is `brotli -q 11 --lgwin=22`, the
setting `mf2-build` compresses catalogs with: almost every visitor downloads the
`.br` file, and `.gz` only reaches clients without brotli (owner, 2026-10-05,
extending the same decision made for B7 in Phase 2). The gzip CLI is not needed
anywhere, which is why the image has no GNU gzip.

**musl.** Everything built as wasm is unaffected by the host's libc, so B1, B5,
B6, B7, B2, B3, B4 and B13 and the browser column of `docs/feature-costs.md`
mean here what they mean anywhere. Every figure taken from a *native* binary —
the stripped `tui-mf2` size, the native canaries, the native column of the
feature-cost table — is a musl figure and is not comparable with a glibc one.

**`CARGO_BUILD_TARGET`.** The image pins it to the musl triple. Every step that
calls cargo unsets it first, because with it set cargo writes to
`target/x86_64-unknown-linux-musl/…`, and `cargo xtask docs` (which wants
`target/debug/mf2`), `native-canaries` and `tui-gate` look in
`target/{debug,release}`. The host is Alpine either way, so unsetting it still
builds musl. Setting it to the empty string is not the same thing: cargo refuses
that with `error: target was empty`.

## The scripts

* `tools/ci/setup.sh [extra …]` — checks the image carries what the checks need
  (`bash`, `xmllint`, `brotli`, `nm`, `wasm-opt`, `wasm-dis`, `node`, `jq`) and
  fails naming anything missing rather than installing it, so a stale image says
  so once instead of being patched at the start of every job; then the
  toolchain `rust-toolchain.toml` pins, and the extras a job asks for
  (`wasm-bindgen`, `twiggy`, `wasmtime`, `trunk`, `fuzz`). Prints every version.
* `tools/ci/versions.sh` — just the versions. `setup.sh` and `report.sh` both
  use it, so the report's header and the job's log cannot disagree.
* `tools/ci/report.sh <artifacts-dir>` — the roll-up.
* `tools/ci/local-only.txt` — what does not run here, and the command for each.
