#!/bin/sh
# A6: does `cargo leptos watch` see a translation edit in a one-crate
# application? Modelled on tools/watch-edit.sh (Phase 7 A6): starts the
# watch, edits fr's `visit-again`, and waits for the server to restart and
# serve the edit, then the revert.
#
#   ./watch.sh without   # the manifest as it is: no watch-additional-files
#   ./watch.sh with      # watch-additional-files = ["locales"]
set -u
mode=$1
here=$(cd "$(dirname "$0")" && pwd)
dir=$here/hello
file=$dir/locales/fr/main.mf2
log=$here/results/watch-$mode.log
port=3960
cd "$dir" || exit 2
cp Cargo.toml "$here/results/.Cargo.toml.bak"
if [ "$mode" = with ]; then
    sed -i 's/^lib-profile-release = "wasm-release"$/lib-profile-release = "wasm-release"\nwatch-additional-files = ["locales"]/' Cargo.toml
fi
grep -n 'watch-additional-files' Cargo.toml || echo "(no watch-additional-files)"
CARGO_BUILD_JOBS=1 setsid cargo leptos watch >"$log" 2>&1 &
pid=$!
listens() { grep -c 'listening on' "$log"; }
wait_listens() { # N SECONDS
    i=0
    while [ "$(listens)" -lt "$1" ]; do
        i=$((i + 1))
        [ $i -gt "$2" ] && return 1
        sleep 1
    done
}
page() { curl -s "http://127.0.0.1:$port/visits?lang=fr"; }
stop() {
    kill -- -"$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    cp "$here/results/.Cargo.toml.bak" "$dir/Cargo.toml"
    rm -f "$here/results/.Cargo.toml.bak"
}
fail=0
wait_listens 1 2400 || { echo "never listened"; stop; exit 2; }
# The watcher is armed after the server first listens ("Creating ignore
# list" follows "listening on"): wait for that line, then 5 s more, so the
# edit cannot land before the watch starts.
i=0
until grep -q 'ignore list from' "$log"; do
    i=$((i + 1))
    [ $i -gt 120 ] && { echo "watcher never armed"; stop; exit 2; }
    sleep 1
done
sleep 5
echo "before: fr /visits mentions 'Revenir' x$(page | grep -c 'Revenir')"
t0=$(date +%s)
sed -i 's/^visit-again = Revenir$/visit-again = Revenir A6/' "$file"
if wait_listens 2 240; then
    echo "restarted $(($(date +%s) - t0)) s after the edit"
    sleep 1
    if page | grep -q 'Revenir A6'; then echo "PASS edit served"; else echo "FAIL edit not served"; fail=1; fi
else
    echo "no restart within 240 s after the edit"
    fail=1
    # Control: is the watcher alive? A Rust source edit must restart it.
    t0=$(date +%s)
    printf '\n// A6 watch control\n' >>"$dir/src/app.rs"
    if wait_listens 2 240; then
        echo "control: a src/app.rs edit restarted it $(($(date +%s) - t0)) s later"
    else
        echo "control: a src/app.rs edit did not restart it either"
    fi
    python3 -c 'import sys; p=sys.argv[1]; s=open(p).read(); open(p,"w").write(s.replace("\n// A6 watch control\n",""))' "$dir/src/app.rs"
fi
sed -i 's/^visit-again = Revenir A6$/visit-again = Revenir/' "$file"
if [ $fail -eq 0 ]; then
    if wait_listens 3 240; then
        sleep 1
        if page | grep -q 'Revenir A6'; then echo "FAIL revert not served"; fail=1; else echo "PASS revert served"; fi
    else
        echo "FAIL no restart after the revert"
        fail=1
    fi
fi
grep -E 'Compiling hello|Dirty hello' "$log" | sed 's/^/  /' | sort | uniq -c
stop
exit $fail
