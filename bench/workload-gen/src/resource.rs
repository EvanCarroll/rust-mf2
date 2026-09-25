//! `.mf2` source files in the working grammar of plans/05-tooling.md §2.
//!
//! Layout of a file:
//!
//! ```text
//! # resource comment (attaches to the frontmatter)
//! @locale en
//! ---
//!
//! # section comment
//! [chat.input]
//!
//! # entry comment
//! @param $count - …
//! unread = You have {$count} unread messages
//! send = Send
//! long = A long value is wrapped with an escaped line break \
//!   and the continuation's indentation is removed.
//! select =
//!   .input {$count :integer}
//!   .match $count
//!   one {{…}}
//!   * {{…}}
//! ```
//!
//! Comments are sized so that they make up 60 % of the source locale's bytes
//! (plans/06 §2); every locale carries the same (English) comments.

use crate::model::{Message, VarKind, Workload};
use crate::rng::Rng;
use crate::shape::apportion;
use crate::text::{Lexicon, capitalize};
use crate::vocab;

/// Values longer than this many bytes are wrapped with escaped line breaks.
const WRAP_AT: usize = 100;
/// Target width of a wrapped line.
const WIDTH: usize = 76;
/// Longest comment line, including `# ` and the line feed.
const COMMENT_LINE: usize = 80;

/// A place a comment can go.
#[derive(Debug, Clone, Copy)]
enum Slot {
    Header,
    Section(usize),
    Entry(usize),
}

/// Comments of one file, shared by all locales.
#[derive(Debug, Clone)]
pub struct Comments {
    /// The file's header; its first line names the file.
    pub(crate) header: Vec<String>,
    /// Per section: `None` = no comment.
    pub(crate) sections: Vec<Option<Vec<String>>>,
    /// Per message index (global): `None` = no comment.
    pub(crate) entries: Vec<Option<Vec<String>>>,
}

/// `@param` description of variable `v` of message `j`.
pub(crate) fn param_description(seed: u64, j: usize, message: &Message, v: usize) -> String {
    let var = &message.vars[v];
    if message.canary {
        return "Build canary; must never reach the client wasm.".to_owned();
    }
    if message.is_select() {
        return "Drives the plural selection; a whole number.".to_owned();
    }
    let mut rng = Rng::stream(seed, "param", ((j as u64) << 3) | v as u64);
    let noun = *rng.pick(vocab::PARAM_NOUNS);
    match var.kind {
        VarKind::Num => match rng.below(3) {
            0 => format!("How many {noun} there are; a whole number."),
            1 => format!("Number of {noun}; never negative."),
            _ => format!("A count of {noun}, shown as digits."),
        },
        VarKind::Str => match rng.below(3) {
            0 => "A display name, as the user typed it.".to_owned(),
            1 => "Free text; may be long and may be in another language.".to_owned(),
            _ => format!("Name of one of the {noun}, as shown in the list."),
        },
        VarKind::Date => "A date and time, shown in the viewer's time zone.".to_owned(),
    }
}

fn write_entry(out: &mut String, key: &str, value: &str) {
    if value.contains('\n') {
        out.push_str(key);
        out.push_str(" =\n");
        for line in value.split('\n') {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        return;
    }
    out.push_str(key);
    out.push_str(" = ");
    if value.len() <= WRAP_AT {
        out.push_str(value);
        out.push('\n');
        return;
    }
    // Break after spaces outside placeholders; the space stays at the end of
    // the line, before the escaping backslash.
    let mut pieces: Vec<&str> = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in value.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ' ' if depth == 0 => {
                pieces.push(&value[start..=i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    pieces.push(&value[start..]);
    let mut column = key.len() + 3;
    let mut line_empty = true;
    for piece in pieces {
        if !line_empty && column + piece.len() > WIDTH {
            out.push_str("\\\n  ");
            column = 2;
        }
        out.push_str(piece);
        column += piece.len();
        line_empty = false;
    }
    out.push('\n');
}

fn write_comment(out: &mut String, lines: &[String]) {
    for line in lines {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
}

/// Writes file `fi` of `locale` from per-message `sources`.
pub fn write_file(
    wl: &Workload,
    fi: usize,
    tag: &str,
    sources: &[String],
    comments: &Comments,
) -> String {
    let file = &wl.files[fi];
    let mut out = String::new();
    write_comment(&mut out, &comments.header);
    out.push_str("@locale ");
    out.push_str(tag);
    out.push_str("\n---\n");
    for (si, section) in file.sections.iter().enumerate() {
        out.push('\n');
        if let Some(head) = &section.head {
            if let Some(lines) = &comments.sections[si] {
                write_comment(&mut out, lines);
            }
            out.push('[');
            out.push_str(head);
            out.push_str("]\n");
        }
        for &j in &section.messages {
            let message = &wl.messages[j];
            if let Some(lines) = &comments.entries[j] {
                if !out.ends_with("\n\n") {
                    out.push('\n');
                }
                write_comment(&mut out, lines);
            }
            for (v, var) in message.vars.iter().enumerate() {
                out.push_str("@param $");
                out.push_str(var.name);
                out.push_str(" - ");
                out.push_str(&param_description(wl.knobs.seed, j, message, v));
                out.push('\n');
            }
            write_entry(&mut out, &message.key, &sources[j]);
        }
    }
    out
}

/// Comment lines filling exactly `bytes` bytes (`# ` + text + LF per line).
fn comment_lines(rng: &mut Rng, lex: &Lexicon, bytes: usize) -> Vec<String> {
    let lines = bytes.div_ceil(COMMENT_LINE).max(1);
    let text = bytes.saturating_sub(3 * lines).max(lines);
    (0..lines)
        .map(|i| {
            let len = text / lines + usize::from(i < text % lines);
            sentence(rng, lex, len)
        })
        .collect()
}

fn sentence(rng: &mut Rng, lex: &Lexicon, len: usize) -> String {
    if len < 3 {
        return lex.fill(rng, len).join(" ");
    }
    let words = lex.fill(rng, len - 1);
    let mut s = capitalize(&words.join(" "));
    s.push('.');
    s
}

/// Plans the comments of file `fi` so that they are 60 % of the source
/// locale's bytes.
pub fn plan_comments(wl: &Workload, fi: usize, en_sources: &[String]) -> Comments {
    plan(wl, fi, "mf2", &|c| write_file(wl, fi, "en", en_sources, c))
}

/// Bytes of `text` on lines that start with `#`.
fn comment_bytes(text: &str) -> usize {
    text.split_inclusive('\n')
        .filter(|line| line.starts_with('#'))
        .map(str::len)
        .sum()
}

/// Plans the comments of file `fi`, written as `ext` by `write`, so that
/// comments are 60 % of the source locale's bytes. Comment lines the format
/// writes on its own (Fluent's `@param` block and section names) count
/// towards the 60 %.
pub(crate) fn plan(
    wl: &Workload,
    fi: usize,
    ext: &str,
    write: &dyn Fn(&Comments) -> String,
) -> Comments {
    let seed = wl.knobs.seed;
    let file = &wl.files[fi];
    let mut rng = Rng::stream(seed, "comments", fi as u64);
    let lex = Lexicon::english();

    // Which slots carry a comment (the empty line before a commented entry
    // counts as non-comment bytes, so decide this first).
    let mut entries: Vec<Option<Vec<String>>> = vec![None; wl.messages.len()];
    let mut sections: Vec<Option<Vec<String>>> = vec![None; file.sections.len()];
    let mut fixed = 0usize;
    for (si, section) in file.sections.iter().enumerate() {
        let is_canary = section.messages.iter().any(|&j| wl.messages[j].canary);
        if is_canary {
            let s = vec!["Build canary (plans/06-size-and-perf.md §3, B6).".to_owned()];
            let e = vec![
                "CI greps the client wasm for this id, its variable name and its text;".to_owned(),
                "any hit fails the build. Every locale has its own canary text.".to_owned(),
            ];
            fixed += s.iter().chain(&e).map(|l| l.len() + 3).sum::<usize>();
            sections[si] = Some(s);
            for &j in &section.messages {
                entries[j] = Some(e.clone());
            }
            continue;
        }
        if section.head.is_some() && rng.chance(80) {
            sections[si] = Some(Vec::new());
        }
        for &j in &section.messages {
            if rng.chance(70) {
                entries[j] = Some(Vec::new());
            }
        }
    }
    let header_first = format!(
        "{}.{ext} — generated by workload-gen ({}); do not edit by hand.",
        file.namespace,
        wl.knobs.summary()
    );
    let mut comments = Comments {
        header: vec![header_first],
        sections,
        entries,
    };
    fixed += comments.header[0].len() + 3;

    // Bytes of the source-locale file with every comment slot empty.
    let bare = {
        let mut bare = comments.clone();
        bare.header.clear();
        for s in bare.sections.iter_mut().flatten() {
            s.clear();
        }
        for e in bare.entries.iter_mut().flatten() {
            e.clear();
        }
        write(&bare)
    };
    let own = comment_bytes(&bare);
    // comments / (comments + rest) = 0.6  ⇔  comments = 1.5 × rest.
    let budget = ((bare.len() - own) * 3 / 2).saturating_sub(fixed + own);

    // Distribute the budget over the open slots by random weights.
    let mut slots: Vec<(Slot, u64)> = vec![(Slot::Header, 8)];
    for (si, s) in comments.sections.iter().enumerate() {
        if matches!(s, Some(lines) if lines.is_empty()) {
            slots.push((Slot::Section(si), 3));
        }
    }
    for section in &file.sections {
        for &j in &section.messages {
            if matches!(&comments.entries[j], Some(lines) if lines.is_empty()) {
                slots.push((Slot::Entry(j), rng.range(1, 5) as u64));
            }
        }
    }
    let weights: Vec<u64> = slots.iter().map(|s| s.1).collect();
    let shares = apportion(budget, &weights);
    for (&(slot, _), &bytes) in slots.iter().zip(&shares) {
        let bytes = bytes.max(8);
        match slot {
            Slot::Header => comments.header.extend(comment_lines(&mut rng, &lex, bytes)),
            Slot::Section(si) => comments.sections[si] = Some(comment_lines(&mut rng, &lex, bytes)),
            Slot::Entry(j) => comments.entries[j] = Some(comment_lines(&mut rng, &lex, bytes)),
        }
    }
    comments
}
