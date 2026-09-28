#!/bin/sh
# Variant 2's report for a dependency that is not local: v1-today, with the
# lint allowed, committed to a throwaway git repository under target/, and
# a consumer crate depending on it by `git = "file://…"` (cargo caps lints
# for such a dependency, as for one from crates.io).
set -eu
cd "$(dirname "$0")"
R=$(cd ../.. && pwd)
G=$PWD/target/gitdep
rm -rf "$G"
mkdir -p "$G/v1dep/src" "$G/consumer/src"
cp -r v1-today/locales v1-today/build.rs "$G/v1dep/"
cp v1-today/src/*.rs "$G/v1dep/src/"
{
    printf '[package]\nname = "v1dep"\nversion = "0.0.0"\nedition = "2024"\n[workspace]\n[features]\n'
    printf 'default = ["allow-52234", "before-crate-path"]\n'
    sed -n '/^\[features\]$/,/^\[dependencies\]$/p' v1-today/Cargo.toml | grep -E '^[a-z0-9-]+ = \[\]$'
    printf '[dependencies]\nmf2 = { path = "%s/crates/mf2", features = ["host-std"] }\n' "$R"
    printf '[build-dependencies]\nmf2-build = { path = "%s/crates/mf2-build" }\n' "$R"
} >"$G/v1dep/Cargo.toml"
(cd "$G/v1dep" && git init -q && git add -A &&
    git -c user.name=probe -c user.email=probe@localhost commit -qm probe)
printf '[package]\nname = "consumer"\nversion = "0.0.0"\nedition = "2024"\n[workspace]\n[dependencies]\nmf2 = { path = "%s/crates/mf2", features = ["host-std"] }\nv1dep = { git = "file://%s/v1dep" }\n' "$R" "$G" >"$G/consumer/Cargo.toml"
printf 'pub fn f() {\n    let _ = v1dep::tr!("hello");\n}\n' >"$G/consumer/src/lib.rs"
cd "$G/consumer"
CARGO_TARGET_DIR="$R/probes/p10-tr-in-crate/target" cargo check
CARGO_TARGET_DIR="$R/probes/p10-tr-in-crate/target" cargo report future-incompatibilities
