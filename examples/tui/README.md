# examples/tui — a trippy-shaped terminal UI

One frame of a network-trace tool in the shape of trippy's terminal UI: a
header with a key-hint bar, a table of hops, the selected hop's details,
chart titles, settings, help, a language menu, flows, an event log and a
status line — 118 messages in English, German, Spanish and French, 126
formatted per frame. It is drawn into a Ratatui `Buffer`, with no terminal,
so that it can be measured.

It is drawn twice:

* `src/ui.rs` — with MF2, written against 2.0 as the user guide's native
  page writes an application: `mf2` with `ratatui`, the messages in
  `locales/` beside the code, a one-line `build.rs`, `tr!` wherever Ratatui
  takes text, and the markup's styles set once as a theme (`ui::theme`).
  1.x's form — a translation crate, a handle passed to everything, a style
  map built for each draw — is at commit `3a296a9`, which the nightly gate
  builds to compare with.
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
cargo xtask tui-gate --book               # add the user guide's native projects
cargo xtask tui-gate --gate --runs 3      # the CI gate: allocations and size
cargo xtask tui-gate --gate --baseline-rev 3a296a920952239a8c62399f2e8d51aad46f84ec
                                          # the nightly gate: and time, against 1.x
```

The gate: the MF2 frame
allocates no more than 1.x's did and no more than the baseline renderer, in
every language; stripped `tui-mf2` is no larger than 1.x's; and, with a
baseline in the rotation, its median time per frame is no more than the
baseline MF2 binary's.

`src/bin/tui-mf2.rs` and `src/bin/tui-upstream.rs` are the measured
binaries: each draws the frame in every language, counts the allocations
of one frame with a counting global allocator, and times a run of frames.
`cargo xtask tui-gate` runs them alternately and takes the median time; the
allocation counts must be the same in every run.
