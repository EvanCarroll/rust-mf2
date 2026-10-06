#!/bin/sh
# Everything a Forgejo job needs before it runs a check, in one place, so a
# package or a tool version is one edit rather than twenty.
#
#   sh tools/ci/setup.sh [extra ...]
#
# Extras, each named by the job that wants it:
#   wasm-bindgen  the CLI, at the version the workspace resolves to
#   twiggy        0.8.0 — bench/b12/check.sh and tools/fmt-check.sh read it
#   wasmtime      49.0.0 into target/tools — conformance layer L4 on wasip1
#   trunk         demo-csr, built by tools/checks/measure.sh
#   fuzz          a nightly toolchain and cargo-fuzz
#
# The runner's image is leptos-builder (Alpine/musl): it brings rustup,
# cargo-binstall, binaryen (wasm-opt, wasm-dis), brotli, node, npm, sccache,
# cargo-leptos, gcc, clang and mold. This script adds what it lacks.
#
# It prints every tool's version at the end. A size figure is only worth
# having if the tools that produced it are named beside it, and tools/ci/report.sh
# copies these lines into the report's header.
set -eu

# The image sets CARGO_BUILD_TARGET=x86_64-unknown-linux-musl. The host is
# Alpine, so unsetting it still builds musl — but artifacts stay in
# target/{debug,release}, where `cargo xtask docs` (target/debug/mf2),
# native-canaries, tui-gate and the nightly differential look for them.
# An empty value is not the same thing: cargo refuses it with "target was
# empty". Every workflow step that calls cargo unsets it the same way.
unset CARGO_BUILD_TARGET

# The image is expected to carry these. leptos-builder's Containerfile brings
# bash (the repository's scripts use arrays and [[ ]]), libxml2-utils for
# xmllint (crates/mf2-cli/tests/xliff.rs *fails* without it rather than
# skipping), brotli (every size figure is `brotli -q 11 --lgwin=22`, the setting
# mf2-build compresses catalogs with) and binutils for nm, which
# `cargo xtask native-canaries` reads symbols with.
#
# Verified, not installed: a stale image should say so plainly here rather than
# be papered over at the start of every job. No gzip — the figures moved to
# brotli (owner, 2026-10-05) and nothing in CI needs the gzip CLI.
echo "setup: the image's tools"
missing=
for tool in bash xmllint brotli nm wasm-opt wasm-dis node jq cargo rustup; do
  command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
# Not a command: the IANA database `mf2-host-std`'s ZONES_HOST reads through
# jiff (`TZDIR`, else /usr/share/zoneinfo). Alpine carries none without
# `tzdata`, and jiff compiles its own copy in only on Windows and wasm
# (`tzdb-bundle-platform`), so without this a named time zone is *Bad Option*
# on the runner and right on a machine that has the database --- which is how
# mf2-fn-datetime's `icu` zone cases passed locally and failed in CI.
[ -d "${TZDIR:-/usr/share/zoneinfo}" ] || missing="$missing tzdata"
if [ -n "$missing" ]; then
  echo "setup: the image is missing:$missing" >&2
  echo "setup: add them to leptos-builder's Containerfile and rebuild the image" >&2
  exit 1
fi

# No argument: the channel, components and targets named in rust-toolchain.toml.
echo "setup: toolchain"
rustup toolchain install
rustup show active-toolchain

for extra in "$@"; do
  case "$extra" in
    wasm-bindgen)
      # Cargo.lock is not committed, so the CLI is installed at whatever
      # version the workspace resolves to. A mismatch between the CLI and the
      # crate is a hard error, not a warning.
      # One version, or stop: with two in the graph, either CLI is wrong for
      # one of them, and that is a decision, not something to guess at.
      # (`sort -V` is not in every busybox, so this counts rather than sorts.)
      want=$(cargo metadata --format-version 1 \
               | jq -r '.packages[] | select(.name == "wasm-bindgen") | .version' \
               | sort -u)
      n=$(printf '%s\n' "$want" | grep -c . || true)
      if [ "$n" -ne 1 ]; then
        echo "setup: the workspace resolves $n wasm-bindgen versions:" >&2
        printf '  %s\n' $want >&2
        echo "setup: pick the one the figures should be taken with" >&2
        exit 1
      fi
      have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
      if [ "$want" = "$have" ]; then
        echo "setup: wasm-bindgen $have is already the resolved version"
      else
        echo "setup: wasm-bindgen $have -> $want"
        cargo install --locked wasm-bindgen-cli --version "$want"
      fi
      ;;
    twiggy)
      # Pinned: bench/b12/README.md's figures were read with this one, and a
      # different twiggy reads a different number. Checked by version, not by
      # presence, so an image that starts shipping twiggy cannot change a figure
      # without anyone noticing.
      want=0.8.0
      have=$(twiggy --version 2>/dev/null | awk '{print $2}' || true)
      if [ "$want" = "$have" ]; then
        echo "setup: twiggy $have is the pinned version"
      else
        echo "setup: twiggy ${have:-absent} -> $want"
        cargo install --locked twiggy --version "$want"
      fi
      ;;
    wasmtime)
      if [ ! -x target/tools/bin/wasmtime ]; then
        cargo install --root target/tools --locked wasmtime-cli --version 49.0.0
      fi
      echo "$(pwd)/target/tools/bin" >> "${GITHUB_PATH:-/dev/null}"
      target/tools/bin/wasmtime --version
      ;;
    trunk)
      command -v trunk >/dev/null 2>&1 || cargo binstall trunk --no-confirm
      trunk --version
      ;;
    fuzz)
      rustup toolchain install nightly --profile minimal
      command -v cargo-fuzz >/dev/null 2>&1 || cargo install --locked cargo-fuzz
      cargo fuzz --version
      ;;
    *)
      echo "setup: unknown extra '$extra'" >&2
      exit 2
      ;;
  esac
done

# The tools that made every figure this run produces.
sh tools/ci/versions.sh
