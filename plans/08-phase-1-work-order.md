# 08 — Phase 1 work order: syntax and data model (layers L1, L2)

Part of the [master plan](00-master-plan.md) (§9, P1). RFC 2119 keywords apply.
Written at the close of Phase 0 (task C4) from
[phase-0-results](phase-0-results.md). **Done**: every task and exit item is
met — see [§ Status at exit](#status-at-exit) and
[phase-1-results](phase-1-results.md); the next phase is
[09-phase-2-work-order](09-phase-2-work-order.md).

Phase 1 builds `mf2-model` and `mf2-syntax`: the spec's data model as Rust
types, a parser to a lossless CST, lowering, the six Data Model Errors, a
serializer and variable analysis — and proves them on all 462 suite messages
(L1, L2), on generated input, under fuzzing, and against `ox_mf2_parser` on the
D1 gate.

## State at the start

| In the tree | Where |
|---|---|
| Workspace, `xtask`, CI skeleton, ledger (every L1/L2 cell `xfail until = "P1"`) | root, `xtask/`, `.forgejo/`, `conformance/` |
| The D1 gate harness with the ox baseline; a slot for a second parser | `bench/parser-gate/` (README: "how Phase 1 adds mf2-syntax") |
| Committed corpora | `bench/corpora/{suite,workload-1600}.json` |
| The spec, the ABNF, the data-model JSON Schema, the suite | `third_party/message-format-wg/` (see the license note below) |

Carried from Phase 0 — **owner decisions** (none blocks A1–A9):

1. Commits and the remote host (decides the CI `runs-on` label).
2. **Spec license** (upstream #1112): if `spec/` must leave the tree, `spec-sync`
   fetches it into a git-ignored cache and every Phase 1 consumer of
   `message.abnf` and `data-model/message.json` (A5 schema test, A8
   generator) reads it from there. Write those consumers against one path
   helper in `mf2-conformance` so the switch is one line.
3. B5 baseline (`idlit`), the B7 restatement and the B9 definition — adopted in
   06 §3 by C2, confirm or overrule.
4. Later-phase items, recorded where they belong: `fixed_decimal` panic paths
   (03 §5.2, before P3 exits), tz database (03 §5.2, P4), `set_locale` URL
   discovery (04 §6, P6), minimum Leptos version (04 §10, P6), the i18n-crate
   recompile cascade (05 §4, P5a), `conformance/REPORT.md` committed or ignored,
   a scheduled networked `spec-sync --check`.
5. **Deferred to after v1, with seams to keep now**: catalog text as JS strings
   ([stretch_goals_after_v1/prob_builtin_strings](stretch_goals_after_v1/prob_builtin_strings.md)).
   The Phase 2 and Phase 3 work orders carry its four seams: opaque `StrRef`
   (02 §5), a catalog-text method on `Sink` (03 §2), DOM writes in one glue
   module (04 §3), catalog loading in one function. Nothing in Phase 1 changes.

Tooling: `cargo-fuzz` needs a nightly toolchain — the fuzz crate is the one
place that uses `+nightly`, stated in its README; everything else stays on the
pinned stable. (`wasmtime` 0.28 on the development machine is too old for L4 —
a P3 concern, not P1.)

## Frozen public types of `mf2-model`

Frozen means: Phase 2 (`mf2-catalog`), Phase 5a (`mf2-build`, `mf2-resource`)
and the ox adapter are written against these names, fields and signatures.
Adding a method is allowed; changing or removing one after A1 lands needs a
change to this document in the same commit. Private representations (marked)
stay free.

Crate rules: `#![no_std]` + `alloc`, `#![forbid(unsafe_code)]`, no
dependencies by default; feature `serde` (JSON per the spec's schema) and
feature `suite-names` (error kinds ↔ the suite's strings, for tests and tools).
Values are kept **as written**: no normalization anywhere in this crate; names
exclude the bidi marks the syntax allows around them (spec "Names and
Identifiers"); `PartialEq` is exact (bytewise), which is what the L2/L3
round-trip properties need. Every data-model type derives `Clone, PartialEq,
Eq, Hash, Debug` and has `into_owned(self) -> Self<'static>`.

```rust
pub use alloc::borrow::Cow;

// ── identities shared with the catalog and the runtime ─────────────────────
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct MsgId(u32);                       // low 24 bits index, high 8 bits chunk (02 §3)
impl MsgId {
    pub const INDEX_BITS: u32 = 24;
    pub const fn new(chunk: u8, index: u32) -> Option<MsgId>;   // None if index ≥ 2^24
    pub const fn from_raw(raw: u32) -> MsgId;
    pub const fn raw(self) -> u32;
    pub const fn chunk(self) -> u8;
    pub const fn index(self) -> u32;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Dir { Ltr = 0, Rtl = 1, Auto = 2 }  // catalog header uses Ltr/Rtl; resolved values may be Auto

// ── errors shared by every crate (the 13 suite kinds + 2) ──────────────────
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum ErrorKind {
    Syntax,                                                   // syntax-error
    VariantKeyMismatch, MissingFallbackVariant, MissingSelectorAnnotation,
    DuplicateDeclaration, DuplicateOptionName, DuplicateVariant,     // data-model errors
    UnresolvedVariable, UnknownFunction, BadSelector,               // resolution errors
    BadOperand, BadOption, BadVariantKey,                           // message-function errors
    UnsupportedOperation, MessageFunctionError,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ErrorClass { Syntax, DataModel, Resolution, MessageFunction }
impl ErrorKind {
    pub const fn class(self) -> ErrorClass;
    #[cfg(feature = "suite-names")] pub fn suite_name(self) -> &'static str;           // "bad-option", …
    #[cfg(feature = "suite-names")] pub fn from_suite_name(s: &str) -> Option<ErrorKind>;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Span { pub start: u32, pub end: u32 }   // byte offsets into the source; start ≤ end ≤ len, char boundaries

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Diagnostic {
    pub kind: ErrorKind,
    pub code: u16,                 // 0 = none; else the frontend's stable detail code (mf2-syntax documents its table)
    pub span: Option<Span>,        // None for models built in code
}
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Diagnostics(/* private: Vec<Diagnostic> */);
impl Diagnostics {
    pub const fn new() -> Self;
    pub fn push(&mut self, d: Diagnostic);
    pub fn iter(&self) -> core::slice::Iter<'_, Diagnostic>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn has(&self, kind: ErrorKind) -> bool;
    pub fn has_class(&self, class: ErrorClass) -> bool;
    pub fn into_vec(self) -> Vec<Diagnostic>;
}

// ── the interchange data model (spec/data-model/README.md, one-to-one) ─────
pub enum Message<'a> { Pattern(PatternMessage<'a>), Select(SelectMessage<'a>) }

pub struct PatternMessage<'a> { pub declarations: Vec<Declaration<'a>>, pub pattern: Pattern<'a> }
pub struct SelectMessage<'a> {
    pub declarations: Vec<Declaration<'a>>,
    pub selectors: Vec<VariableRef<'a>>,
    pub variants: Vec<Variant<'a>>,
}

pub enum Declaration<'a> { Input(InputDeclaration<'a>), Local(LocalDeclaration<'a>) }
pub struct InputDeclaration<'a> { pub name: Cow<'a, str>, pub value: VariableExpression<'a> }  // name == value.arg.name
pub struct LocalDeclaration<'a> { pub name: Cow<'a, str>, pub value: Expression<'a> }

pub struct Variant<'a> { pub keys: Vec<Key<'a>>, pub value: Pattern<'a> }
pub enum Key<'a> { Literal(Literal<'a>), CatchAll(CatchAllKey<'a>) }
pub struct CatchAllKey<'a> { pub value: Option<Cow<'a, str>> }   // always None from MF2 syntax

/// A pattern: a sequence of parts with no empty `Text` and no two adjacent
/// `Text` parts (`push` merges and drops accordingly). The representation
/// stores one part inline, so a placeholder-free message needs no allocation
/// (the D1 target).
pub struct Pattern<'a>(/* private */);
impl<'a> Pattern<'a> {
    pub const fn new() -> Self;
    pub fn from_text(text: Cow<'a, str>) -> Self;          // "" → empty pattern
    pub fn push(&mut self, part: PatternPart<'a>);
    pub fn parts(&self) -> &[PatternPart<'a>];
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn as_simple_text(&self) -> Option<&str>;           // Some for "" and for a single Text part
}
// + FromIterator<PatternPart<'a>>, From<Vec<PatternPart<'a>>>, IntoIterator for &Pattern

#[non_exhaustive]                                          // the spec: not exhaustive
pub enum PatternPart<'a> { Text(Cow<'a, str>), Expression(Expression<'a>), Markup(Markup<'a>) }

#[non_exhaustive]
pub enum Expression<'a> {
    Literal(LiteralExpression<'a>),
    Variable(VariableExpression<'a>),
    Function(FunctionExpression<'a>),
}
pub struct LiteralExpression<'a>  { pub arg: Literal<'a>,     pub function: Option<FunctionRef<'a>>, pub attributes: Attributes<'a> }
pub struct VariableExpression<'a> { pub arg: VariableRef<'a>, pub function: Option<FunctionRef<'a>>, pub attributes: Attributes<'a> }
pub struct FunctionExpression<'a> { pub function: FunctionRef<'a>, pub attributes: Attributes<'a> }
impl<'a> Expression<'a> {
    pub fn function(&self) -> Option<&FunctionRef<'a>>;
    pub fn attributes(&self) -> &Attributes<'a>;
}

pub struct Literal<'a> { pub value: Cow<'a, str> }        // cooked: escapes processed; quoting not kept
pub struct VariableRef<'a> { pub name: Cow<'a, str> }     // without `$`

pub struct FunctionRef<'a> { pub name: Cow<'a, str>, pub options: Options<'a> }  // identifier, e.g. "ns:fn"; no `:` sigil
pub enum OptionValue<'a> { Literal(Literal<'a>), Variable(VariableRef<'a>) }

/// Options in source order. Duplicates are representable, so validation can
/// report Duplicate Option Name; a valid message has none.
pub struct Options<'a>(/* private */);
impl<'a> Options<'a> {
    pub const fn new() -> Self;
    pub fn push(&mut self, name: Cow<'a, str>, value: OptionValue<'a>);
    pub fn iter(&self) -> impl Iterator<Item = (&str, &OptionValue<'a>)>;
    pub fn get(&self, name: &str) -> Option<&OptionValue<'a>>;   // first exact match
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

pub struct Markup<'a> { pub kind: MarkupKind, pub name: Cow<'a, str>, pub options: Options<'a>, pub attributes: Attributes<'a> }
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MarkupKind { Open, Standalone, Close }

/// Attributes in source order; `None` is the spec's `true` (no value).
pub struct Attributes<'a>(/* private */);
impl<'a> Attributes<'a> {
    pub const fn new() -> Self;
    pub fn push(&mut self, name: Cow<'a, str>, value: Option<Literal<'a>>);
    pub fn iter(&self) -> impl Iterator<Item = (&str, Option<&Literal<'a>>)>;
    pub fn get(&self, name: &str) -> Option<Option<&Literal<'a>>>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

pub fn split_identifier(identifier: &str) -> (Option<&str>, &str);   // "ns:name" → (Some("ns"), "name")

// ── the parser boundary (D1's gate and fallback) ───────────────────────────
pub struct Parsed<'src> {
    /// `Some` whenever the source has no syntax error — also when it has
    /// data-model errors, which are then in `diagnostics`.
    pub message: Option<Message<'src>>,
    pub diagnostics: Diagnostics,
}
pub trait Frontend {
    /// `&mut self` lets an implementation keep scratch state across calls (the
    /// gate's "reused" rows); the returned model borrows only the source.
    fn parse<'src>(&mut self, source: &'src str) -> Parsed<'src>;
}
```

Semantics fixed with the types:

* A `Frontend` reports **every** syntax error it can recover to (with spans)
  and, when there is none, every data-model error; `Parsed::message` is then
  `Some`. L1/L2 compare the *set of kinds* with the suite's `expErrors`.
* `serde`: (de)serialization follows `spec/data-model/message.json` — `type`
  tags, `"*"` for catch-all keys, attributes without value as `true`,
  declarations/options/attributes present even when empty. Deserialization
  accepts the schema's relaxations (missing empty collections). Serializing a
  model with duplicate option or attribute names is an error (JSON objects
  cannot hold them).
* Not in `mf2-model`: validation, normalization, analysis (all in
  `mf2-syntax`), and anything the client runtime reads (the catalog). The
  client wasm links `MsgId`, `Dir`, `ErrorKind` only.

## Part A — tasks (A1 first; then A2 → A3 → A4 → A5 in order; A6–A9 once A3 exists)

| Task | Deliverable | Done when |
|---|---|---|
| **A1** `mf2-model` | `crates/mf2-model/` exactly as frozen above; `serde` and `suite-names` features; unit tests for `MsgId` bit layout, `Pattern` invariants (merging, empty text, inline storage), `Options`/`Attributes` order and duplicates, `split_identifier`; `serde` round trip on hand-written fixtures of every node kind. | `cargo test -p mf2-model --all-features` green; `cargo build -p mf2-model --target wasm32-unknown-unknown --no-default-features` green; `forbid(unsafe_code)` |
| **A2** `mf2-syntax` parser → lossless CST | `crates/mf2-syntax/`: byte-oriented, single-pass, **non-recursive** parser (the ABNF has no recursive productions) writing into a flat reusable arena; CST with spans and trivia; error recovery that keeps going; stable diagnostic codes (a documented table, `Diagnostic::code`); `parse_cst(&str) -> Cst`. The simple-message fast path (one scan for `{`, `}`, `\`, leading `.`) short-circuits. | L1: every suite `src` parses; syntax error reported **iff** `expErrors` contains `syntax-error`; `cst.to_string() == src` byte for byte; every span in bounds and on a char boundary |
| **A3** lowering | `parse_model` (implements `Frontend`): CST → `mf2_model::Message` with `Cow` borrowed unless an escape is present; bidi marks around names dropped; values as written. | L2 kinds match (with A4) on all 326 L2-applicable tests; 0 allocations for placeholder-free messages is the target, measured in A9 |
| **A4** validation | The six Data Model Errors, keys and names compared **under NFC** (`unicode-normalization`, quick-check first); callable on a model built in code: `validate(&Message) -> Diagnostics` (spans `None`), and used by `parse_model` with spans. | L2: the set of data-model kinds equals the data-model subset of `expErrors` for every test |
| **A5** serializer | `serialize(&Message) -> Result<String, Error>` (a `Result`: see Status at exit): canonical MF2 source (quoting when needed, escapes, `.input`/`.local`/`.match` layout). | For every L2-clean suite message: `parse(serialize(m)) == m`, and `serialize(m)` is L1-clean; model → JSON validates against `spec/data-model/message.json` |
| **A6** analysis | `analyze(&Message) -> Analysis { externals, locals, markup, functions }`; `externals` in ascending bytewise order of the NFC-normalized names (the manifest's slot order, 05 §3), each with its written spelling; variables used only in options, selectors or markup options included. | unit tests on the suite forms named in 01 §3 L5 (`.input`, `.local` shadowing, option-only variables); agreement with the Phase 0 manifest code on `workload-1600.json` |
| **A7** conformance L1 + L2 | `mf2-conformance` harnesses for L1 and L2 over all 16 files; the ledger flips L1/L2 to `pass` (L2 stays `n/a` for syntax-error tests); `cargo xtask conformance-report` enforces it; ox as a **dev-dependency differential oracle** (`expected ⊆ reported` on syntax + data-model errors; ox's known over-reports on `data-model-errors.json` #0 and #1 recorded). | L1 462/462, L2 326/326 pass; `REPORT.md` shows it; harness red on any regression |
| **A8** generated input + fuzzing | An ABNF-driven generator from the vendored `message.abnf` (through the path helper — see the license note): random well-formed messages; properties: lossless CST, `parse(serialize(m)) == m`, validation deterministic. `fuzz/` with a parser target: no panic, no out-of-bounds, linear time (a time bound per input byte). | property tests run in `cargo test` (bounded cases) and longer in CI nightly-style runs; a fuzz run of ≥ 1 h clean, command recorded |
| **A9** the D1 gate | `bench/parser-gate`: add the `mf2-syntax` adapter (fresh + reused, CST + model, `classify`) to `contenders()`; run the gate; add it to `cargo xtask ci` (release build — decide the CI cost with the owner) or a separate CI job. | `cargo run --release -p parser-gate -- --gate` exits 0: on **every** row ours ≤ 1.05 × ox time (median of ≥ 30) and ≤ ox allocations and bytes, fresh and reused; 462/462 correctness vs ox's 460 |

Order inside the work: A1 unblocks everything. A2 and A3 are the core; write
A4 alongside A3 (lowering sees duplicates). A5 and A6 need only A3. A7 turns
each of A2–A5 into a gate as it lands. A8 and A9 run as soon as A3 exists.

## Exit (master plan §9, P1)

- [x] L1 and L2 100 % over all 462 `src`; `current_phase = "P1"` bumped in the
      exit commit and the harness green
- [x] lossless CST and model round-trip properties hold on generated input
- [x] fuzz run clean (≥ 1 h, command in the results)
- [x] **the D1 gate holds on every row** (target: ≥ 3× on the workload rows,
      zero allocations for placeholder-free messages). If it does not, switch the
      `Frontend` to an `ox_mf2_parser` adapter and carry on — the lowering,
      validation, serializer and analysis are kept — *met: 3.2–15.9× on every
      row, 0 allocations on the placeholder-free model rows; no fallback*
- [x] `mf2-model`'s public types unchanged from this document (or this document
      changed in the same commit as the code, with the reason) — *unchanged;
      additions listed below*
- [x] the Phase 2 work order written from Phase 1's findings
      ([09](09-phase-2-work-order.md))

## Status at exit

Measurements and commands: [phase-1-results](phase-1-results.md).

**Additions to `mf2-model`** (methods and trait impls, which this document
allows; no frozen name, field or signature changed):

* `ErrorKind::ALL` (every kind, in declaration order);
* `Diagnostics`: `From<Vec<Diagnostic>>`, `Extend<Diagnostic>`,
  `IntoIterator for &Diagnostics`;
* `Message::declarations()`, `Declaration::name()`;
* `Pattern::with_capacity`, `Pattern::iter`, `Pattern::into_parts`; `Pattern`'s
  `PartialEq`/`Eq`/`Hash`/`Debug` are implemented over `parts()` (equal parts ⇒
  equal patterns, whatever the internal representation) rather than derived;
* `Options::with_capacity`, `Attributes::with_capacity`, and `FromIterator` for
  both;
* `Parsed` derives `Clone, PartialEq, Eq, Debug`;
* feature `serde`: `Serialize`/`Deserialize` for every node type, not only
  `Message`.

**Departures from the tasks as written:**

* **A5** — `serialize` returns `Result<String, mf2_syntax::Error>`: a model
  built in code can have no MF2 syntax (U+0000 in text or a literal, an invalid
  name, an input declaration whose name differs from its variable, a select
  message without selectors, variants or keys). Every model a parse produces
  serializes.
* **A4** — beyond the frozen semantics: each check is pairwise up to 16 items
  and O(n log n) beyond, after the linear-time test found quadratic validation
  (47.6 s for 50,000 options).
* **A7** — the ledger gained `cargo xtask conformance-report --promote` (flip
  the `xfail` cells a harness passes), and the report now runs the harnessed
  layers: a failing `pass` and a passing `xfail` are violations.
* **A9** — the gate is a separate CI job (`parser-gate`), not a step of `cargo
  xtask ci`: a fat-LTO release build is the cost. **Owner decision:** run it on
  every push, or on a schedule.

**Findings for later phases:**

* `ox_mf2_parser` hangs (and then allocates gigabytes) on `{:` + a
  noncharacter; the oracle runs it in a killable child process with a memory
  limit. It also fails internally on 6.5 % of generated messages that
  `mf2-syntax` parses, and accepts markup as a `.local` value (a syntax error).
  It stays a dev-only oracle and must never see untrusted input.
* Spec readings the suite settles (followed by `mf2-syntax`, not by ox): a
  variant whose keys are all `*` is a fallback whatever its key count; equal
  key lists are duplicate variants whatever their count.
