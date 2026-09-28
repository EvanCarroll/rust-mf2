# examples/tui — a trippy-shaped terminal UI

One frame of a network-trace tool in the shape of trippy's terminal UI: a
header with a key-hint bar, a table of hops, the selected hop's details,
chart titles, settings, help, a language menu, flows, an event log and a
status line — 118 messages in English, German, Spanish and French, 126
formatted per frame. It is drawn into a Ratatui `Buffer`, with no terminal,
so that it can be measured.

It is drawn twice:

* `src/ui.rs` — with MF2, written against the 1.x API as the user guide's
  native page has an application written: a translation crate beside it
  (`i18n/`), `mf2-native`'s `NativeI18n` passed to everything that makes
  text, `mf2-ratatui` for the messages with markup, and the markup's styles
  built for each draw. Phase 10 rewrites it on 2.0 (C8).
* `src/upstream.rs` — the baseline: trippy's own approach, re-implemented
  here from a description of it (a TOML table per message, the locale
  `String` cloned by every lookup, `%{name}` replaced one `str::replace` at a
  time, English plurals, word order assembled in code, key hints bolded by
  slicing the translated word). It is what the MF2 renderer is measured
  against, not a pattern: several of those are visible bugs in the four
  languages.

## Run

```sh
cd examples/tui
cargo run -- --lang fr             # one frame in French, as plain text
cargo run -- --lang fr --upstream  # the same frame, drawn by the baseline
cargo test                         # both renderers, every language
```

## Measure

From the repository root:

```sh
cargo xtask tui-gate                      # allocations, time per frame, stripped sizes
cargo xtask tui-gate --save-baseline DIR  # and keep these binaries
cargo xtask tui-gate --baseline DIR       # alternate with binaries kept earlier
cargo xtask tui-gate --book               # add the user guide's native project
```

`src/bin/tui-mf2.rs` and `src/bin/tui-upstream.rs` are the measured
binaries: each draws the frame in every language, counts the allocations
of one frame with a counting global allocator, and times a run of frames.
`cargo xtask tui-gate` runs them alternately and takes the median time; the
allocation counts must be the same in every run.
