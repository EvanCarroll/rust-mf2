#!/bin/sh
# Phase 10 A3: compiles every variant's call sites, one configuration at a
# time, and writes each compiler output to results/<case>.txt (the
# repository's path replaced by <repo>). Prints PASS/FAIL per case.
#
#   CARGO_BUILD_JOBS=2 ./run.sh [filter]
set -u
cd "$(dirname "$0")"
REPO=$(cd ../.. && pwd)
FILTER=${1:-}
mkdir -p results

run_case() { # name, cargo check args…
    name=$1
    shift
    case "$name" in *"$FILTER"*) ;; *) return ;; esac
    out=results/$name.txt
    if cargo check "$@" >"$out.raw" 2>&1; then status=PASS; else status=FAIL; fi
    # Drop the corpus's own build warnings, which say nothing about tr!.
    grep -v -E '(\[neutral-numbers\]$|Blocking waiting for file lock)' "$out.raw" | sed -e "s#$REPO#<repo>#g" -e "s#$HOME/#~/#g" >"$out"
    rm -f "$out.raw"
    echo "$status $name"
}

# Variant 1: today's form.
for cs in before-unqualified before-crate-path before-use-crate before-super-path \
    after-unqualified after-crate-path after-use-crate after-super-path \
    root-unqualified root-crate-path root-self-path dollar-crate path-mod; do
    run_case "v1.$cs" -p v1-today --features "$cs"
done
run_case v1.consumer -p v1-consumer

# Variant 2: the lint allowed.
for cs in before-crate-path after-crate-path after-use-crate root-crate-path before-unqualified; do
    run_case "v2.$cs" -p v1-today --features "allow-52234,$cs"
done
run_case v2.consumer-of-allowing-dep -p v1-consumer --features dep-allows-52234

# Variant 3: path-addressable shapes of the generated wrapper.
for shape in local export-use hidden-export local-and-export hidden-export-textual; do
    run_case "v3.$shape.none" -p v3-local --lib --no-default-features --features "shape-$shape"
    for cs in before-unqualified before-crate-path before-use-crate before-prelude \
        after-unqualified after-crate-path after-use-crate after-prelude root-unqualified; do
        run_case "v3.$shape.$cs" -p v3-local --lib --no-default-features --features "shape-$shape,$cs"
    done
    for by in by-path by-use by-prelude by-exports; do
        run_case "v3.$shape.consumer-$by" -p v3-consumer --features "shape-$shape,$by"
    done
done

# Variant 4: the env-driven stand-ins (both called from before and after).
run_case v4.check -p v4-env

# 3c in a binary crate: a module before the include, one after, the root.
for shape in local hidden-export hidden-export-textual; do
    run_case "v3.bin-crate.$shape" -p v3-local --bin app --no-default-features --features "shape-$shape"
done

# Variant 5: a generated tr against a glob-imported prelude tr.
for gen in today local; do
    for cs in root-glob after-glob before-glob after-glob-explicit; do
        run_case "v5.$gen.$cs" -p v5-glob --no-default-features --features "gen-$gen,$cs"
    done
done
