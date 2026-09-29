#!/usr/bin/env bash
# Phase 10 C3: the matcher against C3's text half's independent reader
# (`target/p10-c3/evidence.py`, untracked), on 447 tags: 23,906 pairs'
# distances and 3,000 random lists, each against a random application.
# Expected: every answer the same, but where the application offers `und`,
# which the text half's reader fills in and the matcher does not.
#
#   bash probes/p10-c3/oracle.sh
#
# Outputs in target/p10-c3/oracle/; the Rust side runs as a test copied into
# `crates/mf2/tests/` for the run and removed after it.
set -eu
cd "$(dirname "$0")/../.."
out=$(pwd)/target/p10-c3/oracle
mkdir -p "$out"
python3 probes/p10-c3/oracle.py gen "$out"
cp probes/p10-c3/oracle_test.rs crates/mf2/tests/zz_oracle.rs
trap 'rm -f crates/mf2/tests/zz_oracle.rs' EXIT
MF2_ORACLE="$out" CARGO_BUILD_JOBS=3 cargo test -p mf2 --features host-std --test zz_oracle -q
python3 probes/p10-c3/oracle.py compare "$out"
