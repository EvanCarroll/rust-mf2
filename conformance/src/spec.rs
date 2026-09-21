//! Where the vendored specification's files are — the one path helper every
//! consumer of `spec/` goes through (plans/08-phase-1-work-order.md, owner
//! decision 2).
//!
//! Upstream #1112 restricts redistribution of the spec text. If `spec/` has to
//! leave the tree, `cargo xtask spec-sync` will fetch it into a git-ignored
//! cache and [`SPEC_DIR`] changes to that cache: one line, here.

use std::path::{Path, PathBuf};

/// The vendored specification directory, relative to the repository root.
pub const SPEC_DIR: &str = "third_party/message-format-wg/spec";

/// The path of `file` (e.g. `"message.abnf"`, `"data-model/message.json"`)
/// inside the specification directory of the repository at `root`.
pub fn spec_path(root: &Path, file: &str) -> PathBuf {
    root.join(SPEC_DIR).join(file)
}

/// `spec/message.abnf`.
pub const ABNF: &str = "message.abnf";

/// `spec/data-model/message.json` (the data model's JSON Schema).
pub const DATA_MODEL_SCHEMA: &str = "data-model/message.json";
