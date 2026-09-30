#!/bin/sh
# Phase 10 C6: A2's scenarios (plans/18, A2's rows) on the real `mf2` and
# `mf2-build`, with Leptos; A3's call sites in a one-crate application; and
# the edit loop's time with and without the opt-level tip. Writes
# results/real.txt (PASS/FAIL per check) and results/loop.txt.
#
#   CARGO_BUILD_JOBS=3 sh real.sh [loop]
set -u
cd "$(dirname "$0")"
REPO=$(cd ../../.. && pwd)
export CARGO_TARGET_DIR="$PWD/target"
mkdir -p results
W=wasm32-unknown-unknown

fresh_corpus() {
    rm -rf single/locales single/mf2.toml
    cp -r "$REPO/tools/i18n-fixture/locales" single/locales
    cp "$REPO/tools/i18n-fixture/mf2.toml" single/mf2.toml
}

# build NAME CARGO-ARGS…: cargo build -v, JSON messages to NAME.json, the
# human log to NAME.log; the exit status is cargo's.
build() {
    name=$1
    shift
    cargo build -v --message-format=json "$@" >"results/$name.json" 2>"results/$name.log"
}
# The OUT_DIR of package $1's build script in build $2.
outdir() {
    jq -r "select(.reason == \"build-script-executed\" and (.package_id | test(\"#$1@\"))) | .out_dir" \
        "results/$2.json" | tail -1
}
# Whether build $2 ran package $1's build script.
ran() { grep -q "Running \`.*/build/$1-[0-9a-f]*/build-script-build\`" "results/$2.log"; }
say() { # CHECK VERDICT DETAIL
    printf '%s %s — %s\n' "$2" "$1" "$3" | tee -a results/real.txt
}
check() { # CHECK DETAIL CONDITION…
    c=$1
    d=$2
    shift 2
    if "$@"; then say "$c" PASS "$d"; else say "$c" FAIL "$d"; fi
}
has() { ls "$1" 2>/dev/null | grep -q -- "$2"; }
hasnt() { ! has "$@"; }

if [ "${1:-}" = loop ]; then
    # The edit loop: a translation edit, then `cargo build` of the server
    # (`ssr`: the catalogs compressed), alternately without and with
    # `[profile.dev.build-override] opt-level = 2`, one target directory each.
    fresh_corpus
    T2="$PWD/target-opt2"
    build loop-warm0 -p c6-single --features ssr || exit 1
    CARGO_TARGET_DIR=$T2 CARGO_PROFILE_DEV_BUILD_OVERRIDE_OPT_LEVEL=2 \
        build loop-warm2 -p c6-single --features ssr || exit 1
    : >results/loop.txt
    for i in 1 2 3 4 5 6; do
        for opt in 0 2; do
            sed -i "s/^plain = .*/plain = Zapisz $i-$opt/" single/locales/pl/main.mf2
            s=$(date +%s%N)
            if [ $opt = 0 ]; then
                build loop -p c6-single --features ssr
            else
                CARGO_TARGET_DIR=$T2 CARGO_PROFILE_DEV_BUILD_OVERRIDE_OPT_LEVEL=2 \
                    build loop -p c6-single --features ssr
            fi
            e=$(date +%s%N)
            echo "opt$opt $(((e - s) / 1000000)) ms" | tee -a results/loop.txt
        done
    done
    exit 0
fi

: >results/real.txt
fresh_corpus

# Row 2: one crate, `native` — the module and the catalogs, uncompressed.
build native -p c6-single --features native
o=$(outdir c6-single native)
check native "embedded, uncompressed, CORPUS" \
    sh -c "ls '$o' | grep -q '\.mf2b$' && ! ls '$o' | grep -q '\.br$' && grep -q 'static CORPUS' '$o/mf2_generated.rs'"
LANG=pl_PL.UTF-8 "$CARGO_TARGET_DIR/debug/c6-single" >results/native.out 2>&1
check native-run "tr! from before, after and the root" \
    sh -c "grep -q Zapisz results/native.out && grep -q ZQ7-FIXTURE-CANARY-PL results/native.out"
# Row 6/9: the same build again runs nothing.
build native2 -p c6-single --features native
check native-again "no script runs" sh -c "! grep -q 'Running .*build-script-build' results/native2.log"

# Row 2/5/7: `ssr` with Leptos — a web server: compressed catalogs.
build ssr -p c6-single --features ssr
o=$(outdir c6-single ssr)
check ssr "embedded, .br and .gz, CATALOGS" \
    sh -c "ls '$o' | grep -q '\.mf2b\.br$' && ls '$o' | grep -q '\.mf2b\.gz$' && grep -q 'CATALOGS' '$o/mf2_generated.rs'"
# `hydrate` for wasm32, with Leptos — the module only.
build hydrate -p c6-single --lib --target $W --features hydrate
o=$(outdir c6-single hydrate)
wasm="$CARGO_TARGET_DIR/$W/debug/single.wasm"
check hydrate "module only, no catalog, no text in the wasm" \
    sh -c "! ls '$o' | grep -q '\.mf2b' && test -s '$wasm' && ! grep -q ZQ7-FIXTURE-CANARY '$wasm'"
w1=$(sha256sum "$wasm" | cut -c1-16)
# Row 6: both again: nothing runs.
build ssr2 -p c6-single --features ssr
build hydrate2 -p c6-single --lib --target $W --features hydrate
check two-builds-again "no script runs" \
    sh -c "! grep -q 'Running .*build-script-build' results/ssr2.log && ! grep -q 'Running .*build-script-build' results/hydrate2.log"

# Row 10 and the gate: a translation-only edit reruns the script alone and
# leaves the wasm byte-identical.
sed -i 's/^plain = Zapisz$/plain = Zapisz teraz/' single/locales/pl/main.mf2
build hydrate3 -p c6-single --lib --target $W --features hydrate
w2=$(sha256sum "$wasm" | cut -c1-16)
check edit-wasm "wasm $w1 → $w2" \
    sh -c "grep -q 'Running .*c6-single-.*build-script-build' results/hydrate3.log && ! grep -q 'Running .*/build/mf2-[0-9a-f]*/build-script-build' results/hydrate3.log && [ '$w1' = '$w2' ]"
build ssr3 -p c6-single --features ssr
check edit-server "the server's script reruns" ran c6-single ssr3

# Row 9: native → ssr → native: the last runs nothing.
build native3 -p c6-single --features native
build ssr4 -p c6-single --features ssr
build native4 -p c6-single --features native
check toggle "back to native: no script runs" sh -c "! grep -q 'Running .*build-script-build' results/native4.log"

# Row 3: two crates; the translation crate has no features of its own and
# no mf2.toml (note 1: a second build runs nothing).
build two -p c6-app --features native
o=$(outdir c6-i18n two)
check two-crate "the translation crate sees native: CORPUS" \
    sh -c "grep -q 'static CORPUS' '$o/mf2_generated.rs' && ! ls '$o' | grep -q '\.br$'"
LANG=fr_FR.UTF-8 "$CARGO_TARGET_DIR/debug/c6-app" >results/two.out 2>&1
check two-run "fr" grep -q Salut results/two.out
build two2 -p c6-app --features native
check no-mf2-toml "no script runs without mf2.toml" sh -c "! grep -q 'Running .*build-script-build' results/two2.log"

# Row 4: mf2 as a normal dependency (no features) and a build-dependency
# (native, compile): the normal instance's features — the module only.
# Note 4, left for the owner: a corpus whose only placeholder is a string
# still draws the neutral-numbers warning (the conformance ledger needs it
# for a bare placeholder that receives a number).
build host -p c6-host
o=$(outdir c6-host host)
check host-target "the normal instance's: no CORPUS, no catalog" \
    sh -c "test -f '$o/mf2_generated.rs' && ! grep -q 'static CORPUS' '$o/mf2_generated.rs' && ! ls '$o' | grep -q '\.mf2b'"
check neutral-numbers "still warns (an owner question)" grep -q neutral-numbers results/host.log

# run()'s errors: no mf2; datetime-icu without icu-blob.
(cd nomf2 && cargo build -v >../results/nomf2.log 2>&1)
check no-mf2 "run() says so" grep -q "does not name \`mf2\` in its \[dependencies\]" results/nomf2.log
build icu -p c6-single --lib --features mf2/datetime-icu
check icu-blob "run() says so" grep -q 'add `features = \["icu-blob"\]`' results/icu.log

# A3 (plans/19 §12): a module that forgets the import gets rustc's
# suggestion; everything else compiled above.
cargo check -p c6-single --features native,forget >results/forget.log 2>&1
check forget "rustc suggests use crate::tr;" grep -q 'use crate::tr;' results/forget.log

# Row 11: `mf2 check` reads mf2's node (the workspace unifies fn-number in).
"$REPO/target/debug/mf2" -C two/i18n check >results/check.log 2>&1
check mf2-check "clean, from mf2's node" sh -c "grep -q 'nothing to report' results/check.log && ! grep -q 'note:' results/check.log"

# Row 8 and A3 under rust-analyzer: no error in this workspace's crates, and
# no macro call left unresolved anywhere. (It also reports one E0507 in
# mf2's native store, which rustc compiles: rust-analyzer's own.)
rust-analyzer diagnostics . >results/ra.log 2>&1
check rust-analyzer "no error here, no unresolved macro" sh -c \
    "! tr '\r' '\n' <results/ra.log | grep -q -e '/probes/[^:]*: Error' -e 'unresolved-macro-call' -e 'unresolved-proc-macro'"
