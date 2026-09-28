#!/usr/bin/env bash
# A9: every run, in the order the record's figures came from. One step at a
# time, from the root of the tree (the A5 worktree, branch p10-a5-display):
#
#   all.sh STEP
#
# Every figure lands in target/a9/results.tsv (lib, client, case, file, raw,
# gz, br, code, data), every build's log line in target/a9/log.txt.
source "$(dirname "$0")/lib.sh"
cd "$root"
run() { bash "$here/run.sh" "$@"; }

SPLIT="base control"
for form in display debug both tostring; do
  for kind in tr trargs trrich trdyn; do SPLIT="$SPLIT $form-$kind"; done
done
SPLIT="$SPLIT base"
# The demos and `tr`: the forms that separate Display from the text path
# (display against tostring, per type), Debug on the cheapest and the
# heaviest description, and A5's control.
APP="base control display-tr display-trargs display-trrich display-trdyn tostring-tr tostring-trargs
  tostring-trrich tostring-trdyn debug-tr debug-trargs base"

case "$1" in
  # 1. The split, over the four descriptions.
  fixture) run fixture $SPLIT ;;
  tr-view) run tr-view $SPLIT ;;
  # 2. In applications.
  demos)
    shift
    demos=("$@")
    [ ${#demos[@]} = 0 ] && demos=(demo-csr demo-ssr demo-islands)
    for demo in "${demos[@]}"; do run "$demo" $APP; done ;;
  tr) run tr base control display-tr tostring-tr debug-tr debug-trargs base ;;
  # 3. Names kept, for twiggy (what the bytes are).
  named)
    A9_NAMED=1 A9_SHIP=0 run fixture base display-tr tostring-tr debug-tr debug-trargs control
    A9_NAMED=1 A9_SHIP=0 run demo-ssr base display-tr tostring-tr debug-tr debug-trargs control
    # The check on every demo as it is: each must pass.
    A9_NAMED=1 A9_SHIP=0 run demo-csr base
    A9_NAMED=1 A9_SHIP=0 run demo-islands base ;;
  # 4. Ways to shrink it, then ways to remove it (bases, and the errors).
  shrink)
    # S1 changes only what `{}` links (and nothing where `Display` is the
    # stand-in: the fixture, `tr`); S2 only what `{:?}` links.
    bash "$here/libvar.sh" apply s1-writestr
    for c in tr-view demo-csr demo-ssr; do
      A9_LIB=s1-writestr run "$c" base display-tr display-trargs
    done
    bash "$here/libvar.sh" restore
    bash "$here/libvar.sh" apply s2-debug
    A9_LIB=s2-debug run fixture base debug-tr debug-trargs control
    for c in tr-view demo-csr demo-ssr; do
      A9_LIB=s2-debug run "$c" base debug-tr debug-trargs control
    done
    A9_NAMED=1 A9_SHIP=0 A9_LIB=s2-debug run fixture base debug-trargs
    bash "$here/libvar.sh" restore ;;
  remove)
    for v in v1x r1 r1d r2 r2d r3; do
      bash "$here/libvar.sh" apply "$v"
      A9_LIB="$v" run fixture base
      A9_LIB="$v" run tr-view base
      A9_LIB="$v" run demo-csr base
      bash "$here/libvar.sh" restore
    done ;;
  # A5's variant against 1.x in the other two demos, in this tree and lock
  # (their figures against A7's `2fb7f54` builds cross trees).
  remove-demos)
    bash "$here/libvar.sh" apply v1x
    A9_LIB=v1x run demo-ssr base
    A9_LIB=v1x run demo-islands base
    bash "$here/libvar.sh" restore ;;
  errors)
    for v in r1 r1d r2 r2d r3 v1x; do
      bash "$here/libvar.sh" apply "$v"
      A9_CHECK=1 A9_LIB="$v" run demo-ssr display-tr debug-trargs debug-tr
      bash "$here/libvar.sh" restore
    done ;;
  # 5. The silent paths: do they compile (A5, then 1.x), and which Display
  #    each trap reaches (names kept).
  silent)
    A9_CHECK=1 run demo-ssr silent-new silent-traps silent-debug
    # The same on Leptos 0.8 (A7's copy of demo-ssr: target/a9/demos-0-8.py a9)
    A9_CHECK=1 run demo-ssr-08 silent-new silent-traps silent-debug
    bash "$here/libvar.sh" apply v1x
    A9_CHECK=1 A9_LIB=v1x run demo-ssr silent-new silent-traps silent-debug
    A9_CHECK=1 A9_LIB=v1x run demo-ssr-08 silent-new silent-traps silent-debug
    bash "$here/libvar.sh" restore
    A9_NAMED=1 A9_SHIP=0 run demo-ssr silent-traps silent-debug
    # R3: the new paths refused with its message, the traps fmt-free again
    bash "$here/libvar.sh" apply r3
    A9_CHECK=1 A9_LIB=r3 run demo-ssr silent-new silent-traps silent-debug
    A9_NAMED=1 A9_SHIP=0 A9_LIB=r3 run demo-ssr silent-traps
    bash "$here/libvar.sh" restore ;;
  # 6. Each trap alone: what it ships (against R3, where it resolves to the
  #    fmt-free method), and whether the names check sees it.
  traps)
    TRAPS="silent-trap-guard silent-trap-arc silent-trap-refcell silent-trap-refref"
    A9_NAMED=1 run demo-ssr $TRAPS
    bash "$here/libvar.sh" apply r3
    A9_NAMED=1 A9_LIB=r3 run demo-ssr $TRAPS tostring-tr
    bash "$here/libvar.sh" restore
    # What the Debug paths ship, as A5 wrote Debug and as S2 does.
    run demo-ssr silent-debug
    bash "$here/libvar.sh" apply s2-debug
    A9_LIB=s2-debug run demo-ssr silent-debug
    bash "$here/libvar.sh" restore ;;
  # 7. The check on a debug-profile client (no inlining, so a trap's
  #    `Display` keeps its name): demo-ssr, each case, then fmt-check.sh.
  devcheck)
    mkdir -p "$a9/dev-check"
    for c in base silent-trap-guard silent-trap-arc silent-trap-refcell silent-trap-refref \
      display-tr debug-tr debug-trargs tostring-tr; do
      python3 "$here/case.py" apply demo-ssr "$c"
      (cd examples/demo-ssr && CARGO_TARGET_DIR="$a9/t-demo-ssr-dev" nice -n 10 cargo build -q --lib \
        --target wasm32-unknown-unknown --no-default-features --features hydrate --offline \
        > "$a9/dev-check/$c.log" 2>&1) || log "devcheck: $c did not build"
      python3 "$here/case.py" restore demo-ssr
      cp "$a9/t-demo-ssr-dev/wasm32-unknown-unknown/debug/demo_ssr.wasm" "$a9/dev-check/$c.wasm"
    done
    bash "$here/fmt-check.sh" "$a9"/dev-check/*.wasm || true
    # Under R3 the four traps take the fmt-free method: the check passes.
    bash "$here/libvar.sh" apply r3
    python3 "$here/case.py" apply demo-ssr silent-traps
    (cd examples/demo-ssr && CARGO_TARGET_DIR="$a9/t-demo-ssr-dev" nice -n 10 cargo build -q --lib \
      --target wasm32-unknown-unknown --no-default-features --features hydrate --offline \
      > "$a9/dev-check/r3-silent-traps.log" 2>&1) || log "devcheck: r3 silent-traps did not build"
    python3 "$here/case.py" restore demo-ssr
    bash "$here/libvar.sh" restore
    cp "$a9/t-demo-ssr-dev/wasm32-unknown-unknown/debug/demo_ssr.wasm" "$a9/dev-check/r3-silent-traps.wasm.r3"
    bash "$here/fmt-check.sh" "$a9/dev-check/r3-silent-traps.wasm.r3" || true ;;
  # 8. S3, the owner's question (2026-09-28): `Display` through the text the
  #    inherent `to_string()` builds, so that `{}` and the traps reuse its
  #    code. Where `Display` does real work: the Leptos clients.
  s3)
    bash "$here/libvar.sh" apply s3-display-via-string
    A9_LIB=s3-display-via-string run demo-ssr base display-tr display-trargs tostring-tr \
      silent-trap-guard silent-trap-arc silent-trap-refcell silent-trap-refref
    for c in tr-view demo-csr; do
      A9_LIB=s3-display-via-string run "$c" base display-tr display-trargs tostring-tr tostring-trargs
    done
    A9_LIB=s3-display-via-string run demo-islands base display-tr tostring-tr
    bash "$here/libvar.sh" restore ;;
  *) echo "steps: fixture tr-view demos tr named shrink remove remove-demos errors silent traps devcheck s3" >&2; exit 2 ;;
esac
