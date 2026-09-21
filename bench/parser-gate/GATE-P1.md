# Parser gate report

Parsers: ox_mf2_parser 0.14.0-alpha.12, mf2-syntax 0.0.0 (this workspace) (baseline: ox_mf2_parser 0.14.0-alpha.12). rustc 1.98.1 (48a229cea 2026-09-01); opt-level 3. ns/msg and MB/s: median of 31 samples per cell, each ≥ 20 ms, interleaved over every row and parser; best ns: the fastest sample; IQR: interquartile range / median. Allocations: one pass under the counting allocator (exact, load-independent).

| Input | Stage | State | Parser | ns/msg | MB/s | allocs/msg | alloc B/msg | best ns | IQR |
|---|---|---|---|---:|---:|---:|---:|---:|---:|
| 462 suite messages (14.6 KB) | parse → CST | fresh | ox | 820 | 39 | 13.2 | 2,869 | 799 | 2.7 % |
|  |  |  | mf2-syntax | 158 | 200 | 1.3 | 652 | 148 | 3.6 % |
|  | parse → CST | reused | ox | 415 | 76 | 1.1 | 190 | 407 | 2.2 % |
|  |  |  | mf2-syntax | 129 | 246 | 0.0 | 19 | 125 | 1.9 % |
|  | + model + validation | fresh | ox | 1,537 | 21 | 26.3 | 3,490 | 1,506 | 2.1 % |
|  |  |  | mf2-syntax | 305 | 104 | 2.9 | 575 | 293 | 2.0 % |
|  | + model + validation | reused | ox | 1,681 | 19 | 25.3 | 3,082 | 1,631 | 1.6 % |
|  |  |  | mf2-syntax | 284 | 112 | 1.9 | 212 | 268 | 3.5 % |
| 1,600-message workload (43.2 KB) | parse → CST | fresh | ox | 288 | 94 | 8.6 | 1,381 | 280 | 2.9 % |
|  |  |  | mf2-syntax | 67 | 406 | 1.0 | 236 | 64 | 2.0 % |
|  | parse → CST | reused | ox | 199 | 136 | 1.0 | 167 | 195 | 1.5 % |
|  |  |  | mf2-syntax | 45 | 600 | 0.0 | 5 | 42 | 2.0 % |
|  | + model + validation | fresh | ox | 527 | 51 | 11.7 | 1,516 | 516 | 1.5 % |
|  |  |  | mf2-syntax | 86 | 314 | 0.5 | 185 | 84 | 3.0 % |
|  | + model + validation | reused | ox | 610 | 44 | 10.7 | 1,108 | 596 | 2.6 % |
|  |  |  | mf2-syntax | 82 | 328 | 0.3 | 83 | 77 | 5.3 % |
| its 1,256 placeholder-free messages (27.4 KB) | parse → CST | fresh | ox | 199 | 110 | 8.0 | 1,171 | 192 | 2.0 % |
|  |  |  | mf2-syntax | 44 | 492 | 1.0 | 64 | 42 | 4.7 % |
|  | parse → CST | reused | ox | 149 | 146 | 1.0 | 158 | 139 | 2.9 % |
|  |  |  | mf2-syntax | 26 | 832 | 0.0 | 0 | 25 | 4.4 % |
|  | + model + validation | fresh | ox | 340 | 64 | 9.0 | 1,235 | 333 | 2.4 % |
|  |  |  | mf2-syntax | 27 | 818 | 0.0 | 0 | 26 | 3.8 % |
|  | + model + validation | reused | ox | 397 | 55 | 8.0 | 827 | 376 | 1.6 % |
|  |  |  | mf2-syntax | 25 | 862 | 0.0 | 0 | 23 | 7.9 % |

What each row calls:

* ox — parse → CST, fresh: SourceStore per message + parse_source
* ox — parse → CST, reused: shared SourceStore + shared ParseWorkspace + parse_source_session
* ox — + model + validation, fresh: SourceStore per message + parse_source + build_semantic_model + validate_semantics
* ox — + model + validation, reused: shared SourceStore + parse_source + build_semantic_model + validate_semantics
* mf2-syntax — parse → CST, fresh: parse_cst (new arena per message)
* mf2-syntax — parse → CST, reused: one Parser per pass + Parser::parse_cst
* mf2-syntax — + model + validation, fresh: parse_model (fast path, or new arena per message) + validation
* mf2-syntax — + model + validation, reused: one Parser per pass + Parser::parse_model (+ validation)

## Correctness (syntax + Data Model errors)

* ox: **460/462** suite tests exact; 0 of 1600 workload messages report an error.
  * `data-model-errors.json#0` want ["variant-key-mismatch"] got ["missing-fallback-variant", "variant-key-mismatch"] — src ".input {$foo :x} .match $foo * * {{foo}}"
  * `data-model-errors.json#1` want ["variant-key-mismatch"] got ["missing-fallback-variant", "variant-key-mismatch"] — src ".input {$foo :x} .input {$bar :x} .match $foo $bar * {{foo}}"
* mf2-syntax: **462/462** suite tests exact; 0 of 1600 workload messages report an error.

## Gate (plans/05-tooling.md §1)

**PASS** — time ≤ 1.05 × baseline median, allocations and bytes ≤ baseline, every suite test exact.

| Parser | Row | Criterion | Value | Limit | Result |
|---|---|---|---:|---:|---|
| mf2-syntax | suite / parse → CST / fresh | Time | 158 ns | 861 ns | pass |
| mf2-syntax | suite / parse → CST / fresh | Allocs | 598 | 6,110 | pass |
| mf2-syntax | suite / parse → CST / fresh | Bytes | 301,104 | 1,325,319 | pass |
| mf2-syntax | suite / parse → CST / reused | Time | 129 ns | 435 ns | pass |
| mf2-syntax | suite / parse → CST / reused | Allocs | 6 | 490 | pass |
| mf2-syntax | suite / parse → CST / reused | Bytes | 9,008 | 87,567 | pass |
| mf2-syntax | suite / + model + validation / fresh | Time | 305 ns | 1,614 ns | pass |
| mf2-syntax | suite / + model + validation / fresh | Allocs | 1,323 | 12,172 | pass |
| mf2-syntax | suite / + model + validation / fresh | Bytes | 265,845 | 1,612,230 | pass |
| mf2-syntax | suite / + model + validation / reused | Time | 284 ns | 1,765 ns | pass |
| mf2-syntax | suite / + model + validation / reused | Allocs | 871 | 11,711 | pass |
| mf2-syntax | suite / + model + validation / reused | Bytes | 97,909 | 1,423,734 | pass |
| mf2-syntax | workload / parse → CST / fresh | Time | 67 ns | 303 ns | pass |
| mf2-syntax | workload / parse → CST / fresh | Allocs | 1,600 | 13,683 | pass |
| mf2-syntax | workload / parse → CST / fresh | Bytes | 377,984 | 2,210,034 | pass |
| mf2-syntax | workload / parse → CST / reused | Time | 45 ns | 209 ns | pass |
| mf2-syntax | workload / parse → CST / reused | Allocs | 4 | 1,623 | pass |
| mf2-syntax | workload / parse → CST / reused | Bytes | 8,640 | 267,026 | pass |
| mf2-syntax | workload / + model + validation / fresh | Time | 86 ns | 554 ns | pass |
| mf2-syntax | workload / + model + validation / fresh | Allocs | 772 | 18,700 | pass |
| mf2-syntax | workload / + model + validation / fresh | Bytes | 296,184 | 2,425,325 | pass |
| mf2-syntax | workload / + model + validation / reused | Time | 82 ns | 641 ns | pass |
| mf2-syntax | workload / + model + validation / reused | Allocs | 432 | 17,101 | pass |
| mf2-syntax | workload / + model + validation / reused | Bytes | 132,008 | 1,772,525 | pass |
| mf2-syntax | placeholder-free / parse → CST / fresh | Time | 44 ns | 209 ns | pass |
| mf2-syntax | placeholder-free / parse → CST / fresh | Allocs | 1,256 | 10,048 | pass |
| mf2-syntax | placeholder-free / parse → CST / fresh | Bytes | 80,384 | 1,470,506 | pass |
| mf2-syntax | placeholder-free / parse → CST / reused | Time | 26 ns | 157 ns | pass |
| mf2-syntax | placeholder-free / parse → CST / reused | Allocs | 1 | 1,262 | pass |
| mf2-syntax | placeholder-free / parse → CST / reused | Bytes | 64 | 198,506 | pass |
| mf2-syntax | placeholder-free / + model + validation / fresh | Time | 27 ns | 357 ns | pass |
| mf2-syntax | placeholder-free / + model + validation / fresh | Allocs | 0 | 11,304 | pass |
| mf2-syntax | placeholder-free / + model + validation / fresh | Bytes | 0 | 1,550,890 | pass |
| mf2-syntax | placeholder-free / + model + validation / reused | Time | 25 ns | 417 ns | pass |
| mf2-syntax | placeholder-free / + model + validation / reused | Allocs | 0 | 10,049 | pass |
| mf2-syntax | placeholder-free / + model + validation / reused | Bytes | 0 | 1,038,442 | pass |
| mf2-syntax | suite | Correctness | 462 | 462 | pass |
