# P0.11 — node update strategy (settles D7) — RESULT

Probe for `plans/06-size-and-perf.md` §5 (P0.11), `plans/04-leptos-integration.md`
§4 and decision D7 in `plans/00-master-plan.md` §8. The code is throwaway.

## Question

When the locale switches, how should translated nodes update?

* **A.** One `RenderEffect` per node, each tracking a global `ArcTrigger`.
  This is the planning prototype's approach.
* **B.** A library-owned registry. Each node has one slab slot
  `{node, MsgId, args}`, freed in O(1) when its view state drops, and a switch
  walks the slab.

The test setup is 2,000 live translated nodes plus a list that churns 100,000
nodes (mount and unmount). We measure heap growth under churn, switch
latency and bytes per node.

## Verdict

| Threshold (06 §5) | Verdict |
|---|---|
| B: flat heap under churn | **met.** Native: +1,877 B in total over 100k churned nodes. wasm32: +64 KiB once, during the first 10k nodes (slab growth), then flat to 100k. A grows linearly instead: **+151 B per churned node natively, +72 B on wasm32** (+7.2 MB per 100k). The memory is freed only by the next locale switch |
| Switch ≤ 2 × 16.7 ms (33.4 ms) | **met for the library's work.** B's switch script takes 2.3–3.5 ms, or **6.8–6.9 ms at 4× CPU throttle**, with 2,000 live nodes, and churn does not change it. Adding the forced style/layout of the page: B takes 17.9–28.2 ms at 1× (met) and 64–67 ms at 4× (not met). That layout is the same DOM work under A (67–101 ms at 4×) and under any other strategy. It is the cost of re-laying out 2,000 changed text nodes, not of the update mechanism |

**D7 → B** (library-owned registry). B uses about **10× less memory per
node** (44.8 B vs 427 B on wasm32; 37 B vs 797 B native), makes **about 0
allocations per node** (0.01 vs 10), keeps the heap flat under churn, and
switches faster (native 22 µs vs 1.3 ms; wasm at 4×: 6.8 ms vs 12–17 ms, and
vs 32–43 ms for A's first switch after churn). Its code is also smaller:
−4.3 KB raw / −1.9 KB gz in the harness.

## Commands

Everything runs from the repository root. Timings were taken on a shared
8-core machine while five other agents were building (load average 1.5–3.5
during the runs), so re-run on a quiet machine before quoting them as
final. Heap and allocation counts do not depend on load.

```sh
# one command: build + native (A, B) + browser (A, B × 1x, 4x, two runs)
probes/p0-11-node-update/scripts/run.sh

# or the steps:
probes/p0-11-node-update/scripts/build.sh              # native model + wasm A/B (CARGO_BUILD_JOBS=3)
probes/p0-11-node-update/target/cargo/release/p0-11-native both
node probes/p0-11-node-update/browser/run.mjs --runs 2 --json probes/p0-11-node-update/target/browser.json
```

Raw outputs of the runs quoted below: `target/native-run1.txt`,
`target/browser-run2.txt` and `target/browser-run2.json`. The first browser
run (`browser-run1.txt`) used a timing method that turned out to be unfair;
see "Method".

## What was built

* **`native/`** is a native model built on the real `reactive_graph` 0.2.14
  (Leptos 0.8.20) and its executor (`futures-executor`). A counting
  `GlobalAlloc` tracks the heap. tachys cannot create nodes natively (its
  mock DOM is compiled out), so the DOM is a fake: a slab of `String`s, the
  same for both strategies. Node A matches the prototype: create the text,
  then `RenderEffect::new(|prev| { trigger.track(); if prev.is_some() {
  set_text } })`. Node B is a slab slot, removed in `Drop`. Each strategy runs
  in its own process.
* **`wasm/`** is a browser harness built on the **real probe library**
  `probes/p0-01-call-site/crate` (`mf2-probe`), the same code that P0.1
  measures, in a real DOM. Leptos runs in CSR mode (`csr`), with profile
  `wasm-release` (opt-level z, fat LTO). The harness is built twice:
  default = B, `--features a` = A (`mf2-probe/effect`). It mounts 2,000
  `<span>{tr(i)}</span>` and churns `<li>{tr(k)}</li>` rows 50 at a time.
  Each round has its own `Owner`, children of one list owner that is cleaned
  after every batch, as a keyed list or a re-run view closure would do. The
  harness yields to the event loop between batches of 500 nodes (as between
  frames), so the effect tasks spawned by A can finish.
* **`browser/run.mjs`** drives the harness. It uses Playwright from
  `tools/e2e` read-only (through `tools/e2e/lib/browser.mjs`), in Chromium
  headless shell build 1200 (Playwright 1.63.0). Files are served through
  `page.route` (no server). The 4× throttle is CDP
  `Emulation.setCPUThrottlingRate`.

### The library's two strategies (`probes/p0-01-call-site/crate/src/update/`)

* **B (`registry.rs`, the default).** A thread-local slab of
  `Entry { target: Text | (Element, key), id, args: Box<[ArgValue]>, fx: Option<RenderEffect<()>> }`
  with a free list. The view state that a call site holds is **only the slot
  index (4 bytes)**, so its drop glue is one call. `set_catalog` swaps the
  catalog, walks the slab (format, then `set_text` / `set_attribute`), and
  then notifies the trigger for the conversions (`TextProp`,
  `Signal<String>`, `to_string()` under an observer). A node with a
  signal-valued argument also owns one effect that tracks **only its
  arguments**, never the locale trigger. Every path is panic-free
  (`try_borrow`, `get`), with no unsafe code and no fmt.
* **A (`effect.rs`).** One `RenderEffect` per node: track the trigger,
  format, apply. `rebuild` replaces the effect.

## Numbers

### Native (x86_64, 64-bit, release opt-level 3)

| | A (effect per node) | B (registry) |
|---|---|---|
| live heap per node | 797 B | 37 B |
| allocations per node | 10.5 | 0.5 (amortised slab growth) |
| create 2,000 nodes | 1.31 ms | 0.12 ms |
| switch, 2,000 live (median of 21; min–max) | 1.31 ms (1.22–1.61) | 22.5 µs (22.3–24.3) |
| churn 100k nodes: time | 129 ms | 3.2 ms |
| churn 100k nodes: live-heap growth | **+15,099,989 B = 151 B/node, linear** (10k: +1.50 MB … 100k: +15.1 MB) | **+1,877 B total** (flat from 10k) |
| first switch after churn | 6.80 ms (it walks and drops the 100k dead subscribers); median afterwards 1.33 ms | 23.9 µs; median 23.1 µs |
| heap growth after that switch | +2,613 B (the leak is released) | +2,613 B |

### Browser (Chromium, wasm32, two runs; "script" = the `set_catalog` call plus the microtasks until the first and the last live node show the new locale)

| | A 1× | B 1× | A 4× | B 4× |
|---|---|---|---|---|
| live heap per node (wasm32) | 427.1 B | 44.8 B | 427.1 B | 44.8 B |
| allocations per node | 10.02 | 0.01 | 10.02 | 0.01 |
| mount 2,000 nodes | 12.8–15.2 ms | 8.2–10.7 ms | 52.8–65.8 ms | 31.6–33.9 ms |
| **switch script**, median of 21 | 3.9–5.7 ms | **2.3–3.5 ms** | 12.2–17.5 ms | **6.8–6.9 ms** |
| switch script, max | 7.9–11.9 ms | 3.4–14.3 ms | 27.3–35.8 ms | 10.8–10.9 ms |
| forced style/layout after the switch (same DOM work) | 15.2–20.0 ms | 15.7–24.1 ms | 55.4–80.1 ms | 57.0–58.7 ms |
| script + layout | 19.4–25.4 ms | 17.9–28.2 ms | 67.1–101.0 ms | 64.3–66.7 ms |
| churn 100k: time | 423–593 ms | 231–349 ms | 1.70–2.35 s | 0.88–0.89 s |
| churn 100k: heap growth | **+7,212,544 B = 72.1 B/node, linear** | **+65,536 B once (by 10k), then flat** | same as 1× | same as 1× |
| first switch after churn, script | 8.1–10.8 ms | 2.0–2.3 ms | 31.7–43.5 ms | 6.9–7.7 ms |
| heap after that switch | back to +0 | +65,536 | back to +0 | +65,536 |
| harness code size (`wasm-opt -Oz`, raw / gz) | 71,598 / 30,042 | 67,259 / 28,128 | | |

There were no console errors or warnings in any run.

## Method notes and caveats

* **Why run 1 was discarded.** The first browser run timed "switch → next
  macrotask". B happened to meet a rendering step (style/layout ≈ 20–25 ms)
  before the next task, and A did not. That made B look 6× slower (28.5 vs
  4.8 ms). A diagnostic (`target/diag/diag.mjs`) split the time into sync /
  microtasks / forced layout. Both strategies pay the same layout, and B's
  script is shorter. The runner now reports script time and forced-layout
  time separately.
* **What A leaks.** A dropped `RenderEffect` stays in the trigger's
  `SubscriberSet` (a `Weak` that keeps the effect's inner allocation alive)
  until the trigger next fires. Locale switches are rare, so in a long
  session with a churning list (chat log, virtual list) the leak is
  effectively unbounded. Owner children are *not* part of the measured leak:
  `reactive_graph` prunes an owner's child list on the parent's cleanup, and
  both harnesses clean a list owner per batch, as real lists do. Without
  that cleanup, A would also leak `Weak` owner entries. B has no owner or
  effect per node.
* **Conversions still subscribe.** `TextProp` / `Signal<String>` built from a
  `Tr`, and `to_string()` called inside an observer, track the one global
  trigger. Those subscriptions belong to the *consuming* component's effect,
  which exists anyway. If such props sit in churning rows, the same
  dropped-subscriber pattern applies to them (bounded by those props, not by
  every translated node). This is not measured here.
* **Layout is page-dependent.** The harness puts 2,000 spans in a
  flex-wrap container. Any strategy that changes 2,000 visible texts pays
  that relayout, a full remount included. The 33.4 ms threshold is met by
  the update mechanism itself at 4× throttle (≤ 7 ms), but not by the page's
  total frame at 4× (≈ 65 ms).
* The native DOM is fake, so native times exclude DOM writes. The wasm
  numbers include real `Text.nodeValue` writes.
* Only text children were churned. Attributes use the same slot mechanism
  (`Target::Attr`). Nodes with signal-valued arguments (≈ 7 % of sites in
  the reference workload) own one effect in both strategies and were not in
  the churn.

## Recommendations for the real `leptos-mf2`

1. **Adopt B (D7).** Keep the registry thread-local and the slot index as the
   whole view state (4 bytes). Store the node handle and the optional
   argument effect in the slot, so the per-site drop glue stays one call
   (this matters for P0.1's per-site size).
2. Let the switch update the DOM **synchronously** in `set_locale` (walk,
   then notify the conversions). Do not spread the work over thousands of
   microtasks. The first frame after the switch then pays one relayout.
3. For signal-valued arguments, give the node one effect that tracks only its
   arguments, and let the registry handle the locale. Do not subscribe that
   effect to the locale trigger.
4. Follow up in Phase 6 on the conversions' subscriptions under churn: for
   example, a per-owner registration instead of the trigger, or documenting
   `TextProp` from `Tr` inside churning rows.
