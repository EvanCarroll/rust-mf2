//! `cargo xtask docs`: every checked application example in the mdBook user
//! guide, compiled (`plans/15-phase-7-work-order.md` A13).
//!
//! The Markdown under `docs/` is both the mdBook source and the source for
//! examples. A sample that is code — a `rust`,
//! `toml` or `mf2` block — names the file of a small application it belongs
//! to in its info string, and the samples of one application, read in page
//! order, **are** that application:
//!
//! ````text
//! ```rust file=hello/src/lib.rs
//! ```
//! ````
//!
//! Several blocks naming one file are concatenated, so a page can explain a
//! file a piece at a time. A `toml` block marked `merge` is merged into the
//! file the project inherited instead — tables key by key, values replaced —
//! so a page shows only what a variant changes. A block marked `generated` shows a file a command
//! wrote, and must equal what the command wrote; a `sh` block marked
//! `run=<project>` holds `mf2 …` commands, which are run in that project, in
//! order, before its files are written — so the documented `mf2 init` is the
//! one that makes the i18n crate. Nothing is hidden: there are no elided
//! lines, and a code block with no `file=` is refused.
//!
//! A block marked `before` shows code *before* a migration to this library
//! — `leptos-fluent` code, which this workspace does not depend on, so it
//! is never compiled. It is not exempt from checking: its file is written
//! before the project's `run=` commands, a `before` block is only accepted
//! in a project that has them, and those commands (`mf2 convert --from
//! leptos-fluent`) turn it into what the page's `generated` blocks show. A
//! `run=` block marked `status=N` holds commands that must exit with N — a
//! conversion that leaves work for a person exits 1, which is what the page
//! explains next. A `run=` block marked `output=<path>` writes what its
//! commands print (standard output and error, in the order written) to that
//! file of the project, so that a `text` block marked `generated` can show
//! it and be held to it.
//!
//! Every `mf2` block, each read as a file of its own, must be in `mf2
//! fmt`'s form: `mf2 fmt --check` runs on copies of them under
//! `target/docs/fmt` before anything is assembled.
//!
//! The applications are assembled under `target/docs/projects`, their
//! dependencies on this repository's crates pointed at the working tree (the
//! pages write them as the first release will publish them), and each is
//! checked for the targets it runs on, warnings denied, sharing one target
//! directory. A client-only example also publishes its
//! catalogs with `mf2 compile --site`, as its page says to.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::{cargo, run_inherit, run_inherit_env};
use crate::error::{Error, Result};
use crate::fsx;

/// The pages, relative to the repository root, in the order their samples
/// accumulate. The root README's sample is one too.
const PAGES: &[&str] = &[
    "docs/getting-started.md",
    "docs/call-sites.md",
    "docs/delivery-modes.md",
    "docs/switching.md",
    "docs/accessibility.md",
    "docs/native-apps.md",
    "docs/migrating-from-leptos-fluent.md",
    "docs/command-line.md",
    "docs/ecosystem.md",
    "README.md",
];

/// Pages under `docs/` with no samples of their own.
const INDEX_PAGES: &[&str] = &["docs/README.md", "docs/SUMMARY.md", "docs/versioning.md"];

/// This repository's crates, as a documented manifest names them.
const OUR_CRATES: &[(&str, &str)] = &[
    ("mf2", "crates/mf2"),
    ("mf2-native", "crates/mf2-native"),
    ("mf2-ratatui", "crates/mf2-ratatui"),
    ("mf2-build", "crates/mf2-build"),
    ("leptos-mf2", "crates/leptos-mf2"),
    ("mf2-axum", "crates/mf2-axum"),
];

const WASM: &str = "wasm32-unknown-unknown";

/// One `cargo check` of a project.
struct Check {
    target: Option<&'static str>,
    args: &'static [&'static str],
}

/// A server-rendered application: the server natively, the client in wasm.
const SSR_AND_HYDRATE: &[Check] = &[
    Check {
        target: None,
        args: &["--features", "ssr"],
    },
    Check {
        target: Some(WASM),
        args: &["--lib", "--features", "hydrate"],
    },
];

/// A library of samples, rendered both ways.
const LIB_SSR_AND_HYDRATE: &[Check] = &[
    Check {
        target: None,
        args: &["--lib", "--features", "ssr"],
    },
    Check {
        target: Some(WASM),
        args: &["--lib", "--features", "hydrate"],
    },
];

/// One application the samples make.
struct Project {
    name: &'static str,
    /// A project whose assembled files this one starts from.
    base: Option<&'static str>,
    /// Files or directories of the base this one does not have.
    remove: &'static [&'static str],
    checks: &'static [Check],
    /// Publish the catalogs with `mf2 -C <dir> compile --site`, as a static
    /// host would: the crate that holds the messages.
    site: Option<&'static str>,
}

/// Bases before the projects built on them.
const PROJECTS: &[Project] = &[
    // getting-started.md: SSR + hydrate, the documented default.
    Project {
        name: "hello",
        base: None,
        remove: &[],
        checks: SSR_AND_HYDRATE,
        site: None,
    },
    // getting-started.md: the same application kept on Leptos 0.8, the
    // opt-in line (`plans/16-phase-8-work-order.md` A0).
    Project {
        name: "hello-0-8",
        base: Some("hello"),
        remove: &[],
        checks: SSR_AND_HYDRATE,
        site: None,
    },
    // call-sites.md, switching.md and accessibility.md: `tr!` in every
    // position, as components of a library over hello's i18n crate.
    Project {
        name: "calls",
        base: Some("hello"),
        remove: &["src", "i18n/locales"],
        checks: LIB_SSR_AND_HYDRATE,
        site: None,
    },
    // delivery-modes.md: a lazy route, islands, and client-only.
    Project {
        name: "lazy",
        base: Some("hello"),
        remove: &["src/lib.rs"],
        checks: SSR_AND_HYDRATE,
        site: None,
    },
    Project {
        name: "islands",
        base: Some("hello"),
        remove: &["src/lib.rs"],
        checks: SSR_AND_HYDRATE,
        site: None,
    },
    // migrating-from-leptos-fluent.md: Getting started's application as it
    // would be on leptos-fluent, converted, then finished by hand. Its server
    // is hello's, unchanged, and so is its i18n crate, whose messages the
    // page's commands convert.
    Project {
        name: "migrate",
        base: Some("hello"),
        remove: &["src/lib.rs", "i18n/locales"],
        checks: SSR_AND_HYDRATE,
        site: None,
    },
    Project {
        name: "csr",
        base: Some("hello"),
        remove: &["src"],
        checks: &[Check {
            target: Some(WASM),
            args: &[],
        }],
        site: Some("i18n"),
    },
    // command-line.md: the `mf2` commands, run on Getting started's
    // translation crate; the page shows what they print and write, and
    // holds them to it. Nothing to compile beyond `hello`.
    Project {
        name: "cli",
        base: Some("hello"),
        remove: &[],
        checks: &[],
        site: None,
    },
    // command-line.md: the applications `mf2 init --cli` and `--tui` make,
    // as they make them; native-apps.md shows their files, held to what
    // `init` wrote (plans/19 §1.1 and §1.2).
    Project {
        name: "count",
        base: None,
        remove: &[],
        checks: &[Check {
            target: None,
            args: &[],
        }],
        site: None,
    },
    Project {
        name: "hops",
        base: None,
        remove: &[],
        checks: &[Check {
            target: None,
            args: &[],
        }],
        site: None,
    },
    // command-line.md: the web applications `mf2 init --ssr`, `--islands`
    // and `--csr` make, as they make them (plans/05 §6.4).
    Project {
        name: "hello-ssr",
        base: None,
        remove: &[],
        checks: SSR_AND_HYDRATE,
        site: None,
    },
    Project {
        name: "hello-islands",
        base: None,
        remove: &[],
        checks: SSR_AND_HYDRATE,
        site: None,
    },
    Project {
        name: "hello-csr",
        base: None,
        remove: &[],
        checks: &[Check {
            target: Some(WASM),
            args: &[],
        }],
        site: Some("."),
    },
    // native-apps.md's workspace: a library that owns the messages and a
    // terminal UI that draws them (plans/19 §1.3).
    Project {
        name: "trace",
        base: None,
        remove: &[],
        checks: &[Check {
            target: None,
            args: &[],
        }],
        site: None,
    },
];

/// A fenced block from a page.
struct Block {
    page: &'static str,
    /// 1-based line of the opening fence.
    line: usize,
    lang: String,
    file: Option<String>,
    run: Option<String>,
    /// The exit status every command of a `run=` block must have.
    status: i32,
    /// The project file a `run=` block's printed output is written to.
    output: Option<String>,
    generated: bool,
    merge: bool,
    before: bool,
    text: String,
}

impl Block {
    fn at(&self) -> String {
        format!("{}:{}", self.page, self.line)
    }
}

fn fail(message: impl Into<String>) -> Error {
    Error::Docs(message.into())
}

pub(crate) fn run(root: &Path, build: bool) -> Result<()> {
    check_page_list(&root.join("docs"))?;
    let mut blocks = Vec::new();
    for page in PAGES {
        let text = fsx::read_to_string(&root.join(page))?;
        blocks.extend(parse(page, &text)?);
    }
    validate(&blocks)?;

    eprintln!("==> cargo build -p mf2-cli");
    run_inherit(
        &cargo(),
        &[
            OsStr::new("build"),
            OsStr::new("-q"),
            OsStr::new("-p"),
            OsStr::new("mf2-cli"),
        ],
        root,
    )?;
    let mf2 = root.join("target/debug/mf2");

    let out = root.join("target/docs");
    check_mf2_form(&out.join("fmt"), &blocks, &mf2)?;
    let projects = out.join("projects");
    fsx::remove(&projects)?;
    for project in PROJECTS {
        assemble(root, &projects, project, &blocks, &mf2)?;
    }
    // And the files the blocks make, blocks joined and `mf2 convert`'s
    // output included: what a reader who follows the pages ends up with.
    eprintln!("==> mf2 fmt --check on the applications' .mf2 files");
    let status = std::process::Command::new(&mf2)
        .args(["fmt", "--check"])
        .arg(&projects)
        .status()
        .map_err(|e| fail(format!("`mf2 fmt --check` did not run: {e}")))?;
    if !status.success() {
        return Err(fail(format!(
            "the applications' .mf2 files, assembled under {}, are not in `mf2 fmt`'s form (above)",
            projects.display()
        )));
    }
    let compiled = blocks.iter().filter(|b| b.file.is_some()).count();
    let commands = blocks.iter().filter(|b| b.run.is_some()).count();
    eprintln!(
        "==> docs: {} blocks on {} pages; {compiled} are files of {} applications, {commands} run commands (target/docs/projects)",
        blocks.len(),
        PAGES.len(),
        PROJECTS.len()
    );
    if !build {
        return Ok(());
    }

    let target_dir = out.join("target");
    for project in PROJECTS {
        let dir = projects.join(project.name);
        let manifest = dir.join("Cargo.toml");
        for check in project.checks {
            let mut args: Vec<&OsStr> = vec![
                OsStr::new("check"),
                OsStr::new("--manifest-path"),
                manifest.as_os_str(),
            ];
            if let Some(target) = check.target {
                args.push(OsStr::new("--target"));
                args.push(OsStr::new(target));
            }
            args.extend(check.args.iter().map(OsStr::new));
            eprintln!(
                "==> {}: cargo check{}{}",
                project.name,
                check
                    .target
                    .map(|t| format!(" --target {t}"))
                    .unwrap_or_default(),
                check.args.iter().fold(String::new(), |mut s, a| {
                    let _ = write!(s, " {a}");
                    s
                })
            );
            run_inherit_env(
                &cargo(),
                &args,
                &dir,
                &[
                    ("CARGO_TARGET_DIR", target_dir.as_os_str()),
                    // A sample that warns teaches the warning.
                    ("RUSTFLAGS", OsStr::new("-D warnings")),
                ],
            )?;
        }
        if let Some(messages) = project.site {
            let site = out.join("sites").join(project.name).join("i18n");
            fsx::remove(&site)?;
            eprintln!("==> {}: mf2 -C {messages} compile --site", project.name);
            run_inherit(
                mf2.as_os_str(),
                &[
                    OsStr::new("-C"),
                    OsStr::new(messages),
                    OsStr::new("compile"),
                    OsStr::new("--site"),
                    site.as_os_str(),
                ],
                &dir,
            )?;
            if !site.join("index.json").is_file() {
                return Err(fail(format!(
                    "{}: `mf2 compile --site` wrote no index.json",
                    project.name
                )));
            }
        }
    }
    eprintln!("==> docs: every sample compiled");
    Ok(())
}

/// Every page under `docs/` is either read here or an index: a page added
/// and not listed would have samples nothing compiles.
fn check_page_list(docs: &Path) -> Result<()> {
    let tree = fsx::read_tree(docs, ".")?;
    for path in tree.keys() {
        let name = format!("docs/{}", path.trim_start_matches("./"));
        if name.matches('/').count() != 1 || !has_extension(&name, "md") {
            return Err(fail(format!(
                "{name}: only pages belong in docs/ (a sample is a block in a page)"
            )));
        }
        if !PAGES.contains(&name.as_str()) && !INDEX_PAGES.contains(&name.as_str()) {
            return Err(fail(format!(
                "{name} is not in `cargo xtask docs`' page list, so its samples would not be compiled"
            )));
        }
    }
    Ok(())
}

/// The fenced blocks of one page. A fence may be indented (a block inside a
/// list item); its indent is removed from every line of the block.
fn parse(page: &'static str, text: &str) -> Result<Vec<Block>> {
    let mut blocks = Vec::new();
    let mut lines = text.lines().enumerate();
    while let Some((i, line)) = lines.next() {
        let trimmed = line.trim_start();
        let Some(info) = trimmed.strip_prefix("```") else {
            continue;
        };
        let indent = line.len() - trimmed.len();
        let mut words = info.split_whitespace();
        let lang = words.next().unwrap_or("").to_owned();
        let mut block = Block {
            page,
            line: i + 1,
            lang,
            file: None,
            run: None,
            status: 0,
            output: None,
            generated: false,
            merge: false,
            before: false,
            text: String::new(),
        };
        for word in words {
            match word.split_once('=') {
                Some(("file", path)) => block.file = Some(path.to_owned()),
                Some(("run", project)) => block.run = Some(project.to_owned()),
                Some(("status", n)) => {
                    block.status = n.parse().map_err(|_| {
                        fail(format!(
                            "{}: `status={n}` is not an exit status",
                            block.at()
                        ))
                    })?;
                }
                Some(("output", path)) => block.output = Some(path.to_owned()),
                None if word == "generated" => block.generated = true,
                None if word == "merge" => block.merge = true,
                None if word == "before" => block.before = true,
                _ => {
                    return Err(fail(format!(
                        "{}: unknown attribute `{word}` (file=, run=, status=, output=, generated, merge, before)",
                        block.at()
                    )));
                }
            }
        }
        let mut closed = false;
        for (_, body) in lines.by_ref() {
            if body.trim_start().starts_with("```") {
                closed = true;
                break;
            }
            let stripped = if body.len() >= indent && body[..indent].trim().is_empty() {
                &body[indent..]
            } else {
                body.trim_start()
            };
            block.text.push_str(stripped);
            block.text.push('\n');
        }
        if !closed {
            return Err(fail(format!("{}: the block is never closed", block.at())));
        }
        blocks.push(block);
    }
    Ok(blocks)
}

fn project_of(path: &str) -> Option<(&str, &str)> {
    path.split_once('/')
}

fn validate(blocks: &[Block]) -> Result<()> {
    for block in blocks {
        let code = matches!(block.lang.as_str(), "rust" | "toml" | "mf2");
        if code && block.file.is_none() {
            return Err(fail(format!(
                "{}: a `{}` block must name the file it belongs to (file=<project>/<path>), so that it is compiled",
                block.at(),
                block.lang
            )));
        }
        if block.lang.is_empty() {
            return Err(fail(format!(
                "{}: a block needs a language (`text` for output)",
                block.at()
            )));
        }
        if let Some(file) = &block.file {
            let known = project_of(file)
                .is_some_and(|(p, rest)| !rest.is_empty() && PROJECTS.iter().any(|q| q.name == p));
            if !known {
                return Err(fail(format!(
                    "{}: `file={file}` names no project ({})",
                    block.at(),
                    PROJECTS
                        .iter()
                        .map(|p| p.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }
        if let Some(project) = &block.run {
            if block.lang != "sh" || !PROJECTS.iter().any(|p| p.name == project) {
                return Err(fail(format!(
                    "{}: `run=` belongs on a `sh` block and names a project",
                    block.at()
                )));
            }
            for line in commands(block) {
                if !line.starts_with("mf2 ") {
                    return Err(fail(format!(
                        "{}: a `run=` block holds `mf2` commands only, not `{line}`",
                        block.at()
                    )));
                }
            }
        }
        if block.merge && (block.lang != "toml" || block.generated) {
            return Err(fail(format!(
                "{}: `merge` is for a `toml` block that changes an inherited file",
                block.at()
            )));
        }
        if block.generated && block.file.is_none() {
            return Err(fail(format!("{}: `generated` needs file=", block.at())));
        }
        if let Some(path) = &block.output
            && (block.run.is_none()
                || path.is_empty()
                || path.starts_with('/')
                || path.split('/').any(|part| part == ".." || part.is_empty()))
        {
            return Err(fail(format!(
                "{}: `output=` belongs on a `run=` block and names a file inside its project",
                block.at()
            )));
        }
        if block.status != 0 && block.run.is_none() {
            return Err(fail(format!(
                "{}: `status=` belongs on a `run=` block",
                block.at()
            )));
        }
        if block.before {
            let project = block.file.as_deref().and_then(project_of).map(|(p, _)| p);
            let converted =
                project.is_some_and(|p| blocks.iter().any(|b| b.run.as_deref() == Some(p)));
            if !converted || block.generated || block.merge {
                return Err(fail(format!(
                    "{}: `before` is for a file=… of a project whose `run=` commands convert it",
                    block.at()
                )));
            }
        }
    }
    Ok(())
}

/// Every `mf2` block is in `mf2 fmt`'s form (owner, 2026-09-27;
/// `plans/17-phase-9-work-order.md` B9): what a reader copies from the book
/// is what `mf2 fmt --check` accepts. Each block is checked as a file of its
/// own — a block that continues a file shown earlier is read without what
/// came before it, which is how a reader meets it.
fn check_mf2_form(dir: &Path, blocks: &[Block], mf2: &Path) -> Result<()> {
    fsx::remove(dir)?;
    let mut shown = Vec::new();
    for block in blocks.iter().filter(|b| b.lang == "mf2") {
        let name = format!(
            "{}-{}.mf2",
            block.page.trim_start_matches("docs/").replace('/', "-"),
            block.line
        );
        fsx::write(&dir.join(&name), block.text.as_bytes())?;
        shown.push((name, block));
    }
    if shown.is_empty() {
        return Ok(());
    }
    eprintln!(
        "==> mf2 fmt --check on the {} mf2 blocks of the pages",
        shown.len()
    );
    let check = std::process::Command::new(mf2)
        .args(["fmt", "--check"])
        .arg(dir)
        .output()
        .map_err(|e| fail(format!("`mf2 fmt --check` did not run: {e}")))?;
    if check.status.success() {
        return Ok(());
    }
    // Say which blocks, and where each first departs from the form: `mf2
    // fmt` rewrites the copies, which are compared with the pages' text.
    let refused = String::from_utf8_lossy(&check.stderr).into_owned();
    let _ = std::process::Command::new(mf2).arg("fmt").arg(dir).output();
    let mut report = String::new();
    for (name, block) in &shown {
        let formatted = fsx::read_to_string(&dir.join(name))?;
        if formatted != block.text {
            let _ = write!(
                report,
                "\n{}: not in `mf2 fmt`'s form\n{}",
                block.at(),
                diff(&block.text, &formatted).replace("the file:", "mf2 fmt: ")
            );
        }
    }
    Err(fail(format!(
        "`mf2 fmt --check` refuses {} of the pages' mf2 blocks{report}{}",
        String::from_utf8_lossy(&check.stdout)
            .lines()
            .filter(|l| l.starts_with("would format"))
            .count(),
        if refused.trim().is_empty() {
            String::new()
        } else {
            format!("\n{}", refused.trim())
        }
    )))
}

fn commands(block: &Block) -> impl Iterator<Item = &str> {
    block
        .text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn assemble(
    root: &Path,
    projects: &Path,
    project: &Project,
    blocks: &[Block],
    mf2: &Path,
) -> Result<()> {
    let dir = projects.join(project.name);
    if let Some(base) = project.base {
        for (path, bytes) in fsx::read_tree(&projects.join(base), ".")? {
            fsx::write(&dir.join(path.trim_start_matches("./")), &bytes)?;
        }
    }
    for path in project.remove {
        fsx::remove(&dir.join(path))?;
    }
    std::fs::create_dir_all(&dir).map_err(|source| Error::IoAt {
        path: dir.clone(),
        source,
    })?;

    // The code before a migration, which the commands convert.
    let mut before: BTreeMap<&str, Vec<&Block>> = BTreeMap::new();
    for block in blocks.iter().filter(|b| b.before) {
        if let Some((p, path)) = block.file.as_deref().and_then(project_of)
            && p == project.name
        {
            before.entry(path).or_default().push(block);
        }
    }
    for (path, parts) in &before {
        let text = parts
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        fsx::write(&dir.join(path), text.as_bytes())?;
    }

    for block in blocks
        .iter()
        .filter(|b| b.run.as_deref() == Some(project.name))
    {
        let mut printed = Vec::new();
        for line in commands(block) {
            let args: Vec<&OsStr> = line.split_whitespace().skip(1).map(OsStr::new).collect();
            let mut command = std::process::Command::new(mf2);
            command.args(&args).current_dir(&dir);
            let status = if block.output.is_some() {
                run_capturing(command, &mut printed)
            } else {
                command.status()
            }
            .map_err(|e| fail(format!("{}: `{line}` did not run: {e}", block.at())))?;
            if status.code() != Some(block.status) {
                return Err(fail(format!(
                    "{}: `{line}` exited with {status}, not {}",
                    block.at(),
                    block.status
                )));
            }
        }
        if let Some(path) = &block.output {
            fsx::write(&dir.join(path), &printed)?;
        }
    }

    // path → the blocks that make it, in page order.
    let mut files: BTreeMap<&str, Vec<&Block>> = BTreeMap::new();
    for block in blocks.iter().filter(|b| !b.before) {
        if let Some((p, path)) = block.file.as_deref().and_then(project_of)
            && p == project.name
        {
            files.entry(path).or_default().push(block);
        }
    }
    for (path, parts) in files {
        let text = if has_extension(path, "mf2") {
            join_mf2(&parts)
        } else {
            parts
                .iter()
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let target = dir.join(path);
        let merged = parts.iter().filter(|b| b.merge).count();
        if merged != 0 {
            if merged != parts.len() {
                return Err(fail(format!(
                    "{}: {path} is shown both merged and whole",
                    parts[0].at()
                )));
            }
            let base = fsx::read_to_string(&target).map_err(|_| {
                fail(format!(
                    "{}: merges into {path}, which the project does not inherit",
                    parts[0].at()
                ))
            })?;
            let mut table: toml::Table = base
                .parse()
                .map_err(|e| fail(format!("{}: not TOML: {e}", target.display())))?;
            for part in &parts {
                let change: toml::Table = part
                    .text
                    .parse()
                    .map_err(|e| fail(format!("{}: not TOML: {e}", part.at())))?;
                merge(&mut table, change);
            }
            let text =
                toml::to_string(&table).map_err(|e| fail(format!("{}: {e}", target.display())))?;
            fsx::write(&target, text.as_bytes())?;
            continue;
        }
        let generated = parts.iter().filter(|b| b.generated).count();
        if generated != 0 {
            if generated != parts.len() {
                return Err(fail(format!(
                    "{}: {path} is shown both as generated and as written",
                    parts[0].at()
                )));
            }
            let written = fsx::read_to_string(&target).map_err(|_| {
                fail(format!(
                    "{}: shows {path} as generated, but nothing wrote it",
                    parts[0].at()
                ))
            })?;
            if written != text {
                return Err(fail(format!(
                    "{}: shows {path} as generated, but what was written differs:\n{}",
                    parts[0].at(),
                    diff(&text, &written)
                )));
            }
            continue;
        }
        fsx::write(&target, text.as_bytes())?;
    }

    // Point the manifests at the working tree.
    for (path, _) in fsx::read_tree(&dir, ".")? {
        if path.ends_with("Cargo.toml") {
            let file = dir.join(path.trim_start_matches("./"));
            let text = fsx::read_to_string(&file)?;
            let mut text = local_dependencies(root, &file, &text)?;
            // A project stands alone, inside this repository's tree: a
            // manifest `mf2 init` wrote says nothing of a workspace.
            if path == "./Cargo.toml" && !text.contains("[workspace]") {
                text.push_str("\n[workspace]\n");
            }
            fsx::write(&file, text.as_bytes())?;
        } else if has_extension(&path, "toml") {
            let file = dir.join(path.trim_start_matches("./"));
            fsx::read_to_string(&file)?
                .parse::<toml::Table>()
                .map_err(|e| fail(format!("{}: not TOML: {e}", file.display())))?;
        }
    }
    Ok(())
}

/// The blocks of one `.mf2` file, joined the way `mf2 fmt` lays a file out:
/// a blank line between two blocks only where fmt keeps one — before a
/// section head or a comment, and on either side of a message whose value
/// starts on its own line. Each block is in fmt's form already
/// ([`check_mf2_form`]), so the file is too; `assemble` checks it.
fn join_mf2(parts: &[&Block]) -> String {
    let mut text = String::new();
    for part in parts {
        let after_frontmatter = text.ends_with("\n---\n") || text == "---\n";
        if !text.is_empty()
            && (after_frontmatter || ends_in_block(&text) || starts_set_off(&part.text))
        {
            text.push('\n');
        }
        text.push_str(&part.text);
    }
    text
}

/// Whether the entry line `line`, followed by `rest`, starts a value on the
/// next line: `id =` and an indented line after it.
fn is_block_entry<'a>(line: &str, mut rest: impl Iterator<Item = &'a str>) -> bool {
    line.trim_end().ends_with(" =")
        && rest
            .next()
            .is_some_and(|next| next.starts_with(char::is_whitespace) && !next.trim().is_empty())
}

/// Whether a block's text opens with what fmt sets off from what precedes it.
fn starts_set_off(text: &str) -> bool {
    let mut lines = text.lines().skip_while(|l| l.trim().is_empty());
    while let Some(line) = lines.next() {
        if line.starts_with('[') || line.starts_with('#') {
            return true;
        }
        // A property belongs to the entry after it.
        if !line.starts_with('@') {
            return is_block_entry(line, lines);
        }
    }
    false
}

/// Whether text ends with a message whose value starts on its own line.
fn ends_in_block(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    let Some(last) = lines.iter().rposition(|l| {
        !l.trim().is_empty()
            && !l.starts_with(char::is_whitespace)
            && !l.starts_with(['#', '@', '['])
    }) else {
        return false;
    };
    // Only continuation lines after it: the value is the file's last.
    lines[last + 1..]
        .iter()
        .all(|l| l.starts_with(char::is_whitespace) && !l.trim().is_empty())
        && is_block_entry(lines[last], lines[last + 1..].iter().copied())
}

/// Runs `command` with its standard output and error sent into one pipe,
/// appending what it printed, in the order it wrote it, to `printed`.
fn run_capturing(
    mut command: std::process::Command,
    printed: &mut Vec<u8>,
) -> std::io::Result<std::process::ExitStatus> {
    use std::io::Read as _;
    let (mut reader, writer) = std::io::pipe()?;
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(writer.try_clone()?)
        .stderr(writer)
        .spawn()?;
    // The child holds the only writers now (the command's copies go with
    // it): the read ends when the child exits.
    drop(command);
    reader.read_to_end(printed)?;
    child.wait()
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
}

/// `text` with every dependency on one of [`OUR_CRATES`] turned into a path
/// dependency on the working tree, keeping its features.
pub(crate) fn local_dependencies(root: &Path, file: &Path, text: &str) -> Result<String> {
    let mut manifest: toml::Table = text
        .parse()
        .map_err(|e| fail(format!("{}: not TOML: {e}", file.display())))?;
    let rewrite = |table: &mut toml::Table| {
        for key in ["dependencies", "build-dependencies", "dev-dependencies"] {
            let Some(toml::Value::Table(deps)) = table.get_mut(key) else {
                continue;
            };
            for (name, local) in OUR_CRATES {
                let Some(dep) = deps.get_mut(*name) else {
                    continue;
                };
                let path = toml::Value::String(root.join(local).display().to_string());
                match dep {
                    toml::Value::Table(spec) => {
                        spec.remove("version");
                        spec.remove("git");
                        spec.insert("path".into(), path);
                    }
                    other => {
                        let mut spec = toml::Table::new();
                        spec.insert("path".into(), path);
                        *other = toml::Value::Table(spec);
                    }
                }
            }
        }
    };
    rewrite(&mut manifest);
    if let Some(toml::Value::Table(targets)) = manifest.get_mut("target") {
        for (_, target) in targets.iter_mut() {
            if let toml::Value::Table(target) = target {
                rewrite(target);
            }
        }
    }
    toml::to_string(&manifest).map_err(|e| fail(format!("{}: {e}", file.display())))
}

/// `change` into `base`: a table merges key by key, anything else replaces.
fn merge(base: &mut toml::Table, change: toml::Table) {
    for (key, value) in change {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(into)), toml::Value::Table(from)) => merge(into, from),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

/// The first differing line of two texts, for the error.
fn diff(shown: &str, written: &str) -> String {
    let mut shown_lines = shown.lines();
    let mut written_lines = written.lines();
    let mut n = 1;
    loop {
        match (shown_lines.next(), written_lines.next()) {
            (None, None) => return "(they differ only in trailing newlines)".into(),
            (a, b) if a == b => n += 1,
            (a, b) => {
                return format!(
                    "  line {n}, the page: {}\n  line {n}, the file: {}",
                    a.unwrap_or("(end)"),
                    b.unwrap_or("(end)")
                );
            }
        }
    }
}
