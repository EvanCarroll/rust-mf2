# Stretch goal after v1 — narrower string references in NAMES

Status: **deferred until after v1** (decided by the owner, 2026-09-21). Not part
of any phase. This document records the idea, what was measured, why it is
cheap to defer, the seam v1 keeps, and how to re-evaluate it. Part of the
[master plan](../00-master-plan.md) (§9, "Later"); evidence from
[phase-2-results](../phase-2-results.md) §A8 and
`bench/catalog-bench/SIZE-P2.md`.

## 1. The idea

A catalog's NAMES section holds, per pattern or select message, the names of
its variables: the slot names and the `.local` names, used for fallback output
(`{$name}`) and the dynamic named-argument API ([02](../02-catalog-format.md)
§2.6). Format v1 writes each name as a fixed **4-byte** string reference
(`str32`), so any name is one O(1) read. Most of those bytes are zeros: names
are identifiers, the writer puts identifiers first in the string pool, so
their offsets are small.

Two narrower encodings would move fewer bytes over the wire:

| | A. `str16`: fixed 2-byte references | B. varint references (P0.7's prototype) |
|---|---|---|
| Lookup | O(1), as v1 | O(i): skip the `i` names before it |
| Condition | every name in the pool's first 64 KB (writer checks; else it writes `str32`) | none |
| Format | a width flag (1 byte or a header bit) and a new `format_version` major | a new `format_version` major |

## 2. What v1 does, and why

`str32`, chosen in Phase 2 so that every name lookup is O(1): with varints, a
message whose variables all fall back reads its NAMES entry once per variable
(quadratic in its variable count), and the linear-time test of adversarial
catalogs (A7) holds every view to linear work. 02 §2 "Cost bounds" later
accepted repeated work in *hostile* catalogs for records and strings, which
weakens that argument for NAMES; for real messages (a handful of variables)
either encoding costs nothing measurable at runtime.

## 3. What was measured (2026-09-21)

The four locales of the reference workload, production catalogs (COLD and IDS
stripped). `bench/catalog-bench` re-encodes each catalog's NAMES and
re-points every message head, everything else byte for byte, and compresses
the result like the real catalog; **brotli 11 is what B7 budgets and what is
served** ([06](../06-size-and-perf.md) §3). Δ against format v1:

| locale | NAMES (v1) | A. `str16` NAMES | Δ brotli | Δ gz | B. varint NAMES | Δ brotli | Δ gz |
|---|---:|---:|---:|---:|---:|---:|---:|
| en | 1,334 B | 808 B | −89 | −44 | 701 B | **−204** | −52 |
| pl | 1,334 B | 808 B | −46 | −31 | 708 B | −81 | −44 |
| en-XA | 1,334 B | 808 B | −70 | −36 | 708 B | −127 | −63 |
| ar-XB | 1,334 B | 808 B | **+12** | −59 | 708 B | −17 | −74 |

(`str16` also needs a width flag, 1 B, not counted.) Against the served sizes
(18,072 / 24,137 / 21,537 / 18,423 B brotli) the largest saving is 1.1 %
(`en`, varint); B7 passes with room either way. Brotli spreads the savings
unevenly (−204 to +12 B, against −31 to −74 B for gzip): its context
modelling sees the zero bytes differently from locale to locale, and `str16`
recovers only 43–57 % of the varint saving where it helps at all.

`cargo xtask catalog-size` measures both alternatives on every run (the NAMES
table of the report), so these figures stay current.

## 4. Why deferring is cheap

* The catalog is an internal build artifact with no cross-version promise
  (02 F9): a new major version makes readers reject old files, and the
  application's build writes new ones with its wasm. No data migration.
* Only three places know the NAMES bytes: the writer, `Catalog::new`'s
  validation and the `Names` view.

## 5. Seam v1 keeps

**`Names` is the only reader of NAMES.** The runtime asks
`Names::{external, local, var}` for a name and never parses NAMES bytes, so
either alternative changes `mf2-catalog` and nothing that uses it. Keep it
that way.

## 6. When and how to re-evaluate

**Triggers** (any one):

* any other change that needs a new `format_version` major: bundle this in, so
  its cost is a few lines of writer and reader;
* a locale's catalog gets close to B7;
* real application corpora show a larger NAMES share than the reference
  workload (many variables per message, a large identifier pool).

**How**: run `cargo xtask catalog-size` on the corpus in question and read the
NAMES table (brotli column). On the reference workload neither alternative
wins everywhere: B (varint) saves the most but gives up O(1) lookups; A
(`str16`) keeps them but recovers under 60 % of B's saving and costs bytes on
`ar-XB`. Choose on the brotli figures of real corpora; if B, restate 02 §2
"Cost bounds" for NAMES.
