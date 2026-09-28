#!/usr/bin/env bash
# Runs the accepted cases, then each refused case, keeping each compiler
# output in results/. From probes/p10-args/.
set -u
mkdir -p results
cargo run --quiet --example accepted > results/accepted.txt 2>&1; echo "accepted: exit $?" | tee -a results/accepted.txt
for f in neither neither-generic option; do
  cargo build --quiet --example refused --features "$f" > "results/refused-$f.txt" 2>&1
  echo "refused-$f: exit $?" | tee -a "results/refused-$f.txt"
done
