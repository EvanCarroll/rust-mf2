# D1 baseline: `ox_mf2_parser` 0.14.0-alpha.12 on the committed corpora

Phase 0 task P0.12. These numbers **replace the audit table** in
[plans/05-tooling.md](../../plans/05-tooling.md) §1 as the baseline of the D1
gate. Merging them into the plans is task C2. The machine-readable record of
this run is [`baseline.json`](baseline.json), which holds every quartile,
allocation totals and build info.

## Command

```sh
CARGO_BUILD_JOBS=3 cargo run --release -p parser-gate -- --gate --json bench/parser-gate/baseline.json
```

The defaults are 31 samples per cell, each ≥ 20 ms, interleaved over all 12
rows; 100 ms warm-up per cell; the workspace `release` profile (opt-level 3,
fat LTO). With ox alone, `--gate` reports *baseline only* and exits 0. Re-run
the same command in a quiet window to refresh the absolute times. Allocation
figures do not change between runs.

## Machine and load

* Intel Core i7-1165G7 (4 cores / 8 threads, up to 4.7 GHz), Linux 6.12 x86_64,
  rustc 1.98.1 (stable, pinned by `rust-toolchain.toml`).
* **Load caveat.** The machine is shared with five other agents running cargo
  builds. This run was held until the 1-minute load average fell below 3.
  During the ~10 s run the load average was 2.94 → 2.72 (5-minute: 6.1). At
  loads of 5–12, observed earlier in the same session, the medians were
  1.5–3× higher; the fastest samples ("best") were less affected. The IQR
  column shows the noise that was left.
* Run finished 2026-09-20 23:52 local (unix 1789966338).

## Baseline table

ns/msg and MB/s are the median of 31 samples. allocs/msg and alloc B/msg come
from one pass under the counting allocator. They are exact and do not depend
on load.

| Input | Stage | State | ns/msg | MB/s | allocs/msg | alloc B/msg | best ns | IQR |
|---|---|---|---:|---:|---:|---:|---:|---:|
| 462 suite messages (14.6 KB) | parse → CST | fresh | 1,011 | 31 | 13.2 | 2,869 | 912 | 18.0 % |
| | parse → CST | reused | 508 | 62 | 1.1 | 190 | 457 | 12.0 % |
| | + model + validation | fresh | 1,912 | 17 | 26.3 | 3,490 | 1,706 | 13.4 % |
| | + model + validation | reused | 2,025 | 16 | 25.3 | 3,082 | 1,858 | 9.2 % |
| 1,600-message workload (43.2 KB) | parse → CST | fresh | 361 | 75 | 8.6 | 1,381 | 331 | 9.5 % |
| | parse → CST | reused | 232 | 117 | 1.0 | 167 | 212 | 13.7 % |
| | + model + validation | fresh | 659 | 41 | 11.7 | 1,516 | 618 | 8.8 % |
| | + model + validation | reused | 733 | 37 | 10.7 | 1,108 | 683 | 8.2 % |
| its 1,256 placeholder-free messages (27.4 KB) | parse → CST | fresh | 258 | 85 | 8.0 | 1,171 | 240 | 10.8 % |
| | parse → CST | reused | 173 | 126 | 1.0 | 158 | 160 | 7.0 % |
| | + model + validation | fresh | 447 | 49 | 9.0 | 1,235 | 419 | 8.5 % |
| | + model + validation | reused | 488 | 45 | 8.0 | 827 | 448 | 8.8 % |

What ox calls in each row:

* fresh: a new `SourceStore` per message + `parse_source` (+
  `build_semantic_model` + `validate_semantics` for the model row, run only
  when there is no syntax diagnostic);
* CST, reused: one `SourceStore::with_capacity(n)` + one `ParseWorkspace` per
  pass, then `parse_source_session` per message;
* model, reused: one `SourceStore::with_capacity(n)` per pass. ox's model API
  needs an owned `ParseResult`, so `parse_source` still builds its own
  workspace per call.

### What `mf2-syntax` must meet (gate rule 1, per pass)

Allocation counts and bytes are compared **exactly**, per pass. Here are the
limits (from `baseline.json`):

| Row | allocs ≤ | bytes ≤ | time ≤ (1.05 × median, at this run's load) |
|---|---:|---:|---:|
| suite / CST / fresh | 6,110 | 1,325,319 | 1,062 ns |
| suite / CST / reused | 490 | 87,567 | 533 ns |
| suite / model / fresh | 12,172 | 1,612,230 | 2,008 ns |
| suite / model / reused | 11,711 | 1,423,734 | 2,126 ns |
| workload / CST / fresh | 13,683 | 2,210,034 | 379 ns |
| workload / CST / reused | 1,623 | 267,026 | 244 ns |
| workload / model / fresh | 18,700 | 2,425,325 | 692 ns |
| workload / model / reused | 17,101 | 1,772,525 | 770 ns |
| placeholder-free / CST / fresh | 10,048 | 1,470,506 | 271 ns |
| placeholder-free / CST / reused | 1,262 | 198,506 | 182 ns |
| placeholder-free / model / fresh | 11,304 | 1,550,890 | 469 ns |
| placeholder-free / model / reused | 10,049 | 1,038,442 | 512 ns |

The time limits are for orientation only. The gate compares medians taken in
the same run, so it does not depend on this table's absolute times.

## Correctness (the "strictly more correct" baseline)

**ox: 460/462** suite tests exact on syntax + Data Model errors, reproducing
the audit. Both misses over-report `missing-fallback-variant` next to the
expected `variant-key-mismatch`:

* `data-model-errors.json` #0: `.input {$foo :x} .match $foo * * {{foo}}`
* `data-model-errors.json` #1: `.input {$foo :x} .input {$bar :x} .match $foo $bar * {{foo}}`

ox reports no error on any of the 1,600 workload messages.

## Findings

1. **The one-`SourceStore`-per-message caveat is large for the CST row.**
   With fresh state, most of ox's allocations are per-call setup, not parsing.
   `parse_source` builds a new `ParseWorkspace` on every call, and the new
   store adds its `Vec` plus the copy of the source. Reusing the store and
   workspace drops the CST row from 13.2 to **1.1** allocs/msg on the suite and
   from 8.6 to **1.0** on the workload, and halves the time (1,011 → 508 ns;
   361 → 232 ns). The one remaining allocation per message is
   `SourceStore::add`'s owned copy of the source (placeholder-free: 1,262
   allocs = 1,256 copies + 6). So in reused mode `mf2-syntax` must allocate at
   most about one block per message, and for a placeholder-free message its
   target is zero.
2. **For the model row, reuse gains nothing in time.** ox cannot reuse its
   workspace there (see above). Only the store is saved (−1 alloc/msg,
   −408 B/msg on every corpus), and the reused model row is 6–11 % *slower* than fresh on
   all three corpora (2,025 vs 1,912; 733 vs 659; 488 vs 447 ns). A plausible
   cause is heap locality: the shared store keeps every source alive, so the
   per-call workspace no longer recycles the same hot blocks. This has not
   been investigated further. It does not affect the gate, which compares each
   row with the same row.
3. **Placeholder-free messages are only 1.4× cheaper than the average workload
   message for ox** (258 vs 361 ns CST fresh), and they still cost 8.0
   allocs/msg fresh and 1.0 reused. There is no simple-message fast path.
   That is the headroom behind the ≥ 3× target.

## Comparison with the audit table (plans/05 §1)

| Input | Stage (fresh) | audit ns/msg | now ns/msg | audit allocs / B | now allocs / B |
|---|---|---:|---:|---:|---:|
| 462 suite messages (14.6 KB) | CST | 1,435 | 1,011 | 13.2 / 2,869 | 13.2 / 2,869 |
| | + model | 2,884 | 1,912 | 26.3 / 3,490 | 26.3 / 3,490 |
| 1,600 workload | CST | 502 | 361 | 8.3 / 1,458 | 8.6 / 1,381 |
| | + model | 917 | 659 | 11.2 / 1,581 | 11.7 / 1,516 |
| placeholder-free | + model | 632 | 447 | 9.0 / 1,332 | 9.0 / 1,235 |

* **Suite rows use identical input.** The allocation figures match the audit
  to the byte, which confirms that the harness measures the same thing as
  `probes/audit/ox-parser-bench`.
* **Workload rows use a different corpus.** The audit's inline generator gave
  55.9 KB (≈ 35 B/msg, 1,281 placeholder-free). The committed
  `workload-1600.json` is 43.2 KB (27.0 B/msg, 1,256 placeholder-free), with
  markup, `.match` and 1–4-variable messages in the proportions of
  plans/06 §2. Shorter messages give fewer bytes per message. The allocation
  counts differ slightly for the same reason.
* **Times are ~30 % below the audit's.** In the same quiet window as this run,
  the audit probe itself (built from `probes/audit/ox-parser-bench`, three
  runs alternated with the harness) gave 840–1,148 ns/msg for the suite CST
  row and 1,454–2,590 ns for the suite model row. The harness gave 911–999 ns
  and 1,731–1,901 ns. The two tools agree within the noise, so the gap to the
  audit table comes from the machine's state during the planning session, not
  from the method. The remaining differences in method: the audit probe used
  atomic allocation counters, `codegen-units = 1`, and 200 back-to-back
  iterations (not interleaved).
