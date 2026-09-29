//! `cargo xtask release [--publish]`: everything a release needs, checked in
//! one command (`plans/17-phase-9-work-order.md` A7; owner question 4).
//!
//! Without `--publish` it is the dry run CI runs on every change:
//!
//! 1. **at once**, so one run names every cheap problem: the tree clean
//!    (`git status --porcelain`); the version's changelog entry (A6); the
//!    20's metadata (A1);
//! 2. the package audit (A4, `cargo xtask package --check`: quick, so an
//!    unaudited file stops the run before the long steps), which also
//!    leaves each `.crate` in `target/package`;
//! 3. the names on crates.io — the interrupted initial release has five
//!    crates at 1.0.0, so their owners establish the project identity until
//!    `mf2` itself is published. Later releases use `mf2`'s owners. Every
//!    other name must be free or owned by the same account. A crate that
//!    already carries this version counts as **released** when
//!    the published `.crate` ships what this tree's does, file for file but
//!    the commit cargo records (a publish that stopped part way, as
//!    crates.io's limit on new crates stops one), and as a problem
//!    otherwise: a published version cannot be replaced;
//! 4. the public API against each crate's latest published version
//!    (`cargo-semver-checks`, pinned, installed into `target/tools`), with
//!    the features its documentation presents — skipped for a crate with
//!    nothing published; `--baseline-rev` compares with a git revision
//!    instead;
//! 5. `cargo xtask ci` (the committed API listings of A2 among its steps);
//! 6. the packages' own tests from their `.crate` files (A4, `--test`);
//! 7. the documentation as docs.rs builds it (A5);
//! 8. the MSRV build (A3);
//! 9. `cargo publish --workspace --dry-run`, the released crates excluded;
//! 10. the tree as it was at the start: no step may have written to it.
//!
//! With `--publish` — the owner's, never CI's: it refuses when `CI` is set —
//! the same, then `cargo publish --workspace` with cargo's own stored login,
//! the released crates excluded. When crates.io refuses a new crate for its
//! rate limit (HTTP 429, "try again after …"), the command waits until the
//! time it names and publishes the rest; run again after any other stop, it
//! takes up where the publish left off. This command stores and reads no
//! token of its own. It prints the tag to
//! create and what to update once the crates are on crates.io; it never
//! tags or pushes.
//!
//! crates.io's API is asked about each name once, a second apart (its
//! crawler policy), and about the owners of a name that is taken; the
//! sparse index (`index.crates.io`) says whether a version is out, and its
//! `.crate` is fetched from `static.crates.io` into `target/release-check`.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::{BufRead as _, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::cmd::{cargo, run_capture, run_inherit};
use crate::error::{Error, Result};
use crate::{changelog, packages};

/// The pinned `cargo-semver-checks` (crates.io), installed into
/// `target/tools` when missing.
const SEMVER_CHECKS: &str = "0.50.0";

/// How this command names itself to crates.io's API, which requires a user
/// agent.
const USER_AGENT: &str = "rust-mf2-release-check (cargo xtask release)";

/// How many times a publish stopped by crates.io's rate limit is resumed.
const PUBLISH_ATTEMPTS: u32 = 40;

/// The wait when crates.io's refusal names no time it can be read from.
const DEFAULT_WAIT: Duration = Duration::from_mins(10);

/// The crate whose owners are "ours" once a release is published.
const FACADE: &str = "mf2";

/// The five packages published before crates.io rate-limited the initial
/// release. They identify the owners until the facade has its first version.
const PARTIAL_INITIAL_RELEASE: [&str; 5] = [
    "mf2-model",
    "mf2-resource",
    "mf2-syntax",
    "mf2-catalog",
    "mf2-macros",
];

/// Features left out of a crate's cargo-semver-checks run, by crate.
/// cargo-semver-checks builds a placeholder crate that depends on the one it
/// checks, so a dependency's feature cannot be passed, and 1.0.0's `mf2`
/// does not compile its Leptos layer (`leptos`, or a mode) without a line
/// from `leptos-mf2` (`leptos-mf2/leptos-0-9`, refused as "not allowed to
/// contain slashes"). Since Phase 10's B1 the layer is `mf2`'s own, held by
/// its `api.txt`; B5 checks it per mode, against baseline feature sets that
/// spell 1.0.0's line. `native` (Phase 10 B2) is not a feature of 1.0.0's
/// `mf2` at all: 1.x's native crate, `mf2-native`, was never published. What
/// it adds is held by `api.txt` too, and B5 lists it as its own mode.
const SEMVER_WITHOUT: [(&str, &[&str]); 1] = [(
    "mf2",
    &[
        "leptos",
        "ssr",
        "static-locale",
        "mark-fallback-lang",
        "native",
    ],
)];

/// What the command does after its checks.
pub(crate) struct Options {
    /// Publish to crates.io (the owner's, never CI's).
    pub(crate) publish: bool,
    /// Carry on with a dirty tree (a dry run only): for trying a change
    /// before committing it.
    pub(crate) allow_dirty: bool,
    /// Compare the public API with this git revision instead of the
    /// published version.
    pub(crate) baseline_rev: Option<String>,
}

fn fail(message: impl Into<String>) -> Error {
    Error::Release(message.into())
}

pub(crate) fn run(root: &Path, options: &Options) -> Result<()> {
    if options.publish && std::env::var_os("CI").is_some() {
        return Err(fail(
            "--publish refuses to run in CI (`CI` is set): publishing is the owner's, \
             with the owner's own crates.io login",
        ));
    }

    eprintln!("==> release: the tree, the changelog, the metadata");
    let metadata = packages::metadata(root)?;
    let version = packages::version(&metadata)
        .ok_or_else(|| fail("cargo metadata names no `mf2` with a version"))?
        .to_owned();
    let changelog = changelog::read(root)?;
    let earlier = changelog::earlier(&changelog, &version);
    let tree = status(root)?;
    let mut problems = Vec::new();
    if !tree.is_empty() {
        let line = format!("the tree is not clean:\n{}", indent(&tree));
        if options.allow_dirty {
            eprintln!("release: carrying on (--allow-dirty): {line}");
        } else {
            problems.push(line);
        }
    }
    problems.extend(changelog::problems(&changelog, &version));
    problems.extend(packages::problems(&metadata));
    refuse(&problems)?;

    eprintln!("==> cargo xtask package --check");
    crate::package::run(root, true, false)?;

    eprintln!("==> release: the names on crates.io");
    let local = local_digests(root, &version)?;
    let names = lookup_all(root, &version)?;
    refuse(&name_problems(
        &names,
        &version,
        earlier.first().copied(),
        &local,
    ))?;
    let mut released = released(&names, &local);
    eprintln!(
        "release: {version}; {}{}",
        match earlier.first() {
            None => "the first release: every name free or this release's".to_owned(),
            Some(previous) => format!("after {previous}: every name free or ours"),
        },
        if released.is_empty() {
            String::new()
        } else {
            format!("; already on crates.io, identical: {}", released.join(", "))
        }
    );

    semver(root, &names, &version, options.baseline_rev.as_deref())?;

    eprintln!("==> cargo xtask ci");
    crate::ci::run(root)?;
    eprintln!("==> cargo xtask package --check --test");
    crate::package::run(root, true, true)?;
    eprintln!("==> cargo xtask docs-rs");
    crate::docs_rs::run(root)?;
    eprintln!("==> cargo xtask msrv");
    crate::msrv::run(root, false)?;

    if released.len() == packages::PUBLISHED.len() {
        eprintln!("==> cargo publish: nothing left to publish; all 20 are on crates.io");
    } else {
        let mut dry = publish_args(&released);
        dry.push("--dry-run".to_owned());
        if options.allow_dirty {
            dry.push("--allow-dirty".to_owned());
        }
        eprintln!("==> cargo {}", dry.join(" "));
        run_inherit(&cargo(), &dry.map_os(), root)?;
    }

    let after = status(root)?;
    if after != tree {
        return Err(fail(format!(
            "a step changed the tree; before:\n{}\nafter:\n{}",
            indent(&tree),
            indent(&after)
        )));
    }

    let tag = format!("v{version}");
    if !options.publish {
        eprintln!(
            "==> release: the dry run of {version} passed; publish with `cargo xtask release --publish`"
        );
        return Ok(());
    }
    let mut attempt = 1;
    while released.len() < packages::PUBLISHED.len() {
        let args = publish_args(&released);
        eprintln!("==> cargo {}", args.join(" "));
        let Some(wait) = publish(root, &args)? else {
            break;
        };
        if attempt == PUBLISH_ATTEMPTS {
            return Err(fail(format!(
                "crates.io still refuses new crates after {PUBLISH_ATTEMPTS} waits; run \
                 `cargo xtask release --publish` again later: it publishes the rest"
            )));
        }
        attempt += 1;
        eprintln!(
            "==> release: crates.io's limit on new crates; waiting {} s, then publishing the rest",
            wait.as_secs()
        );
        std::thread::sleep(wait);
        released = released_now(root, &version, &local)?;
    }
    eprintln!(
        "==> release: {version} is published.\n\
         Next, by hand:\n\
         * tag it:  git tag -a {tag} -m \"rust-mf2 {version}\"  (and push the tag where the \
         repository lives)\n\
         * after this coordinated release, update the README and getting-started page to \
         say that all crates are available on crates.io"
    );
    Ok(())
}

/// `git status --porcelain`, untracked files included: empty when clean.
fn status(root: &Path) -> Result<String> {
    let out = run_capture(
        OsStr::new("git"),
        &["status", "--porcelain", "--untracked-files=all"].map_os(),
        root,
        &[],
    )?;
    Ok(String::from_utf8_lossy(&out).trim_end().to_owned())
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `&str`s as the `&OsStr`s `cmd` takes.
trait MapOs {
    fn map_os(&self) -> Vec<&OsStr>;
}

impl<S: AsRef<str>> MapOs for [S] {
    fn map_os(&self) -> Vec<&OsStr> {
        self.iter().map(|s| OsStr::new(s.as_ref())).collect()
    }
}

// ----- the names -----

/// What crates.io has under a name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Registered {
    Free,
    Taken {
        /// Every version published, yanked ones included.
        versions: Vec<String>,
        /// The owners' logins (users and teams).
        owners: Vec<String>,
        /// The content digest of the version being released, when it is
        /// published (`package::content_digest`).
        this: Option<String>,
    },
}

/// Each of the 20, as crates.io has it now.
fn lookup_all(root: &Path, version: &str) -> Result<Vec<(&'static str, Registered)>> {
    let mut out = Vec::new();
    for (i, name) in packages::PUBLISHED.iter().enumerate() {
        if i > 0 {
            std::thread::sleep(Duration::from_secs(1));
        }
        out.push((*name, lookup(root, name, version)?));
    }
    Ok(out)
}

/// The content digest of each published crate's `.crate`, as `cargo
/// package` left it in `target/package` for this tree.
fn local_digests(root: &Path, version: &str) -> Result<BTreeMap<&'static str, String>> {
    let dir = root.join("target").join("package");
    let mut out = BTreeMap::new();
    for name in packages::PUBLISHED {
        let path = dir.join(format!("{name}-{version}.crate"));
        out.insert(name, crate::package::content_digest(&path)?);
    }
    Ok(out)
}

/// The crates already on crates.io at this version, identical to this
/// tree's packages.
fn released(
    found: &[(&'static str, Registered)],
    local: &BTreeMap<&str, String>,
) -> Vec<&'static str> {
    found
        .iter()
        .filter(|(name, r)| {
            matches!(r, Registered::Taken { this: Some(sum), .. } if local.get(name) == Some(sum))
        })
        .map(|(name, _)| *name)
        .collect()
}

/// [`released`], asking crates.io again (after a publish stopped).
fn released_now(
    root: &Path,
    version: &str,
    local: &BTreeMap<&str, String>,
) -> Result<Vec<&'static str>> {
    let mut out = Vec::new();
    for name in packages::PUBLISHED {
        if published_digest(root, name, version)?.as_ref() == local.get(name) {
            out.push(name);
        }
    }
    Ok(out)
}

/// The content digest of `name` at `version` on crates.io, if published.
fn published_digest(root: &Path, name: &str, version: &str) -> Result<Option<String>> {
    if !in_index(name, version)? {
        return Ok(None);
    }
    let dir = root.join("target").join("release-check");
    std::fs::create_dir_all(&dir).map_err(|source| Error::IoAt {
        path: dir.clone(),
        source,
    })?;
    let path = dir.join(format!("{name}-{version}.crate"));
    let url = format!("https://static.crates.io/crates/{name}/{name}-{version}.crate");
    let out = path.to_string_lossy().into_owned();
    let args = [
        "-fsSL",
        "--proto",
        "=https",
        "--max-time",
        "120",
        "-A",
        USER_AGENT,
        "-o",
        &out,
        &url,
    ];
    run_capture(OsStr::new("curl"), &args.map_os(), Path::new("."), &[])?;
    crate::package::content_digest(&path).map(Some)
}

/// Whether the sparse index lists `name` at `version`.
fn in_index(name: &str, version: &str) -> Result<bool> {
    let prefix = match name.len() {
        1 => "1".to_owned(),
        2 => "2".to_owned(),
        3 => format!("3/{}", &name[..1]),
        _ => format!("{}/{}", &name[..2], &name[2..4]),
    };
    let url = format!("https://index.crates.io/{prefix}/{name}");
    let args = [
        "-sS",
        "--proto",
        "=https",
        "--max-time",
        "60",
        "-A",
        USER_AGENT,
        "-w",
        "\n%{http_code}",
        &url,
    ];
    let out = run_capture(OsStr::new("curl"), &args.map_os(), Path::new("."), &[])?;
    let out = String::from_utf8_lossy(&out);
    let (body, code) = out.rsplit_once('\n').unwrap_or(("", &out));
    match code.trim() {
        "404" => return Ok(false),
        "200" => {}
        other => return Err(fail(format!("{url}: HTTP {other}"))),
    }
    Ok(body
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .any(|v| v["vers"] == version))
}

fn lookup(root: &Path, name: &str, version: &str) -> Result<Registered> {
    let url = format!("https://crates.io/api/v1/crates/{name}");
    let (code, body) = get(&url)?;
    match code {
        404 => return Ok(Registered::Free),
        200 => {}
        _ => return Err(fail(format!("{url}: HTTP {code}"))),
    }
    let versions: Vec<String> = body["versions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["num"].as_str().map(str::to_owned))
        .collect();
    std::thread::sleep(Duration::from_secs(1));
    let url = format!("{url}/owners");
    let (code, body) = get(&url)?;
    if code != 200 {
        return Err(fail(format!("{url}: HTTP {code}")));
    }
    let owners = body["users"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|u| u["login"].as_str().map(str::to_owned))
        .collect();
    let this = if versions.iter().any(|v| v == version) {
        published_digest(root, name, version)?
    } else {
        None
    };
    Ok(Registered::Taken {
        versions,
        owners,
        this,
    })
}

/// A GET of crates.io's API: the status and the JSON body.
fn get(url: &str) -> Result<(u16, Value)> {
    let args = [
        "-sS",
        "--proto",
        "=https",
        "--max-time",
        "60",
        "-A",
        USER_AGENT,
        "-w",
        "\n%{http_code}",
        url,
    ];
    let out = run_capture(OsStr::new("curl"), &args.map_os(), Path::new("."), &[])?;
    let out = String::from_utf8_lossy(&out);
    let (body, code) = out.rsplit_once('\n').unwrap_or(("", &out));
    let code: u16 = code
        .trim()
        .parse()
        .map_err(|_| fail(format!("{url}: no HTTP status in curl's output")))?;
    let body = serde_json::from_str(body).unwrap_or(Value::Null);
    Ok((code, body))
}

/// Every problem with the names as `found`, for releasing `version` after
/// `previous` (`None`: the first release), this tree's packages having the
/// checksums `local`.
fn name_problems(
    found: &[(&str, Registered)],
    version: &str,
    previous: Option<&str>,
    local: &BTreeMap<&str, String>,
) -> Vec<String> {
    let mut out = Vec::new();
    let facade = found.iter().find(|(n, _)| *n == FACADE).map(|(_, r)| r);
    let ours: Option<Vec<String>> = match (previous, facade) {
        (None, _) => None,
        (
            Some(previous),
            Some(Registered::Taken {
                versions, owners, ..
            }),
        ) => {
            if versions.iter().any(|v| v == previous) {
                Some(owners.clone())
            } else {
                partial_initial_owners(found, previous).or_else(|| {
                    out.push(format!(
                        "{FACADE}: crates.io does not have {previous}, the release before this \
                         one in CHANGELOG.md, so its owners are not known to be ours"
                    ));
                    None
                })
            }
        }
        (Some(previous), _) => partial_initial_owners(found, previous).or_else(|| {
            out.push(format!(
                "{FACADE}: not on crates.io, though CHANGELOG.md says {previous} was released"
            ));
            None
        }),
    };
    for (name, registered) in found {
        let Registered::Taken { owners, this, .. } = registered else {
            continue;
        };
        if let Some(sum) = this {
            // This version is out: this release, stopped part way, or not.
            if local.get(name) != Some(sum) {
                out.push(format!(
                    "{name}: {version} is already on crates.io and is not this tree's \
                     package (a published version cannot be replaced)"
                ));
            }
            continue;
        }
        match ours.as_deref() {
            None if previous.is_none() => out.push(format!(
                "{name}: taken on crates.io (owners: {}), and nothing of ours is published yet",
                owners.join(", ")
            )),
            Some(ours) if !owners.iter().any(|o| ours.contains(o)) => out.push(format!(
                "{name}: owned on crates.io by {}, none of whom owns a published crate from this project",
                owners.join(", ")
            )),
            _ => {}
        }
    }
    out
}

fn partial_initial_owners(found: &[(&str, Registered)], previous: &str) -> Option<Vec<String>> {
    PARTIAL_INITIAL_RELEASE.iter().find_map(|expected| {
        found.iter().find_map(|(name, registered)| {
            if name != expected {
                return None;
            }
            match registered {
                Registered::Taken {
                    versions, owners, ..
                } if versions.iter().any(|v| v == previous) => Some(owners.clone()),
                _ => None,
            }
        })
    })
}

fn refuse(problems: &[String]) -> Result<()> {
    if problems.is_empty() {
        return Ok(());
    }
    Err(fail(format!(
        "{} problem(s):\n{}",
        problems.len(),
        indent(&problems.join("\n"))
    )))
}

// ----- publishing -----

/// `cargo publish --workspace`, leaving out the crates already released.
fn publish_args(released: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = ["publish", "--workspace"].map(str::to_owned).into();
    for name in released {
        args.extend(["--exclude".to_owned(), (*name).to_owned()]);
    }
    args
}

/// Runs `cargo <args>`, its output shown as it comes. `Ok(None)` when it
/// published everything; `Ok(Some(wait))` when crates.io's rate limit on new
/// crates stopped it, with how long to wait.
fn publish(root: &Path, args: &[String]) -> Result<Option<Duration>> {
    let shown = format!("cargo {}", args.join(" "));
    let mut child = Command::new(cargo())
        .args(args)
        .current_dir(root)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| Error::Spawn {
            program: shown.clone(),
            source,
        })?;
    let mut said = String::new();
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines() {
            let line = line?;
            eprintln!("{line}");
            said.push_str(&line);
            said.push('\n');
        }
    }
    let status = child.wait()?;
    if status.success() {
        return Ok(None);
    }
    if !said.contains("429 Too Many Requests") {
        return Err(Error::CommandFailed {
            command: shown,
            status: status.to_string(),
            stderr: String::new(),
        });
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    Ok(Some(match retry_after(&said) {
        // A few seconds past the time named, and never less than that.
        Some(at) => Duration::from_secs(at.saturating_sub(now) + 5),
        None => DEFAULT_WAIT,
    }))
}

/// The time in crates.io's refusal ("Please try again after Sat, 26 Sep 2026
/// 03:15:30 GMT"), as seconds since the Unix epoch.
fn retry_after(said: &str) -> Option<u64> {
    let rest = &said[said.find("try again after ")? + "try again after ".len()..];
    let date = rest.split(" GMT").next()?;
    // "Sat, 26 Sep 2026 03:15:30"
    let mut parts = date.split_whitespace().skip(1);
    let day: u64 = parts.next()?.parse().ok()?;
    let month = parts.next()?;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == month)? as u64
        + 1;
    let year: u64 = parts.next()?.parse().ok()?;
    let mut hms = parts.next()?.split(':').map(|n| n.parse::<u64>().ok());
    let (h, m, s) = (hms.next()??, hms.next()??, hms.next()??);
    Some(days_from_civil(year, month, day)? * 86_400 + h * 3600 + m * 60 + s)
}

/// Days from 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// algorithm), for dates from 1970 on.
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    let y = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe).checked_sub(719_468)
}

// ----- the public API against the published version -----

fn semver(
    root: &Path,
    names: &[(&str, Registered)],
    version: &str,
    baseline_rev: Option<&str>,
) -> Result<()> {
    let presented = crate::docs_rs::presented(root)?;
    // Each crate's baseline: the newest version published before this one
    // (a crate already out at this version is not its own baseline).
    let baseline = |name: &str| -> Option<String> {
        names
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, r)| match r {
                Registered::Taken { versions, .. } => versions
                    .iter()
                    .filter(|v| *v != version)
                    .max_by_key(|v| {
                        v.split('.')
                            .map(|n| n.parse::<u64>().unwrap_or(0))
                            .collect::<Vec<_>>()
                    })
                    .cloned(),
                Registered::Free => None,
            })
    };
    let compared: Vec<_> = presented
        .iter()
        .filter(|p| baseline_rev.is_some() || baseline(&p.name).is_some())
        .collect();
    if compared.is_empty() {
        eprintln!(
            "==> cargo-semver-checks: skipped — nothing is published to compare with \
             (the first release)"
        );
        return Ok(());
    }
    let tool = install_semver_checks(root)?;
    let mut failed = Vec::new();
    for p in compared {
        if p.proc_macro {
            // cargo-semver-checks reads a library's rustdoc JSON, which a
            // proc-macro crate does not have ("no crates with library
            // targets"); its promise is its macros' names, held by
            // `api.txt`, and their forms, by the `tr!` tests.
            eprintln!(
                "==> cargo-semver-checks: {} skipped (a proc-macro crate)",
                p.name
            );
            continue;
        }
        let dropped = SEMVER_WITHOUT
            .iter()
            .find(|(n, _)| *n == p.name)
            .map_or(&[][..], |(_, f)| *f);
        let features: Vec<&str> = p
            .features
            .iter()
            .map(String::as_str)
            .filter(|f| !dropped.contains(f))
            .collect();
        let mut args: Vec<String> = ["semver-checks", "--package", &p.name]
            .map(str::to_owned)
            .into();
        // The features docs.rs documents: what `api.txt` lists and 1.x
        // promises. Without one of these flags, cargo-semver-checks would
        // guess its own set.
        args.push(
            if p.all_features {
                "--all-features"
            } else if p.no_default_features {
                "--only-explicit-features"
            } else {
                "--default-features"
            }
            .to_owned(),
        );
        if !features.is_empty() {
            args.extend(["--features".to_owned(), features.join(",")]);
        }
        if let Some(target) = &p.default_target {
            // cargo-semver-checks runs rustc with `--cap-lints=allow`, which
            // on Rust 1.98 silences the "unsupported crate type" warning
            // cargo's wasm32 target probe relies on, so a wasm32 build fails
            // before it starts ("output of --print=file-names missing").
            // The crate is checked on the host instead, which is the same
            // API while no item of it is gated on the target (checked here;
            // for `mf2-host-web` its host and wasm32 listings were identical,
            // 2026-09-25).
            let gated = target_gated(&root.join("crates").join(&p.name))?;
            if !gated.is_empty() {
                return Err(fail(format!(
                    "{} is documented for {target}, which cargo-semver-checks cannot build, \
                     and its API depends on the target ({}), so the host's is not the same",
                    p.name,
                    gated.join(", ")
                )));
            }
        }
        if let Some(rev) = baseline_rev {
            args.extend(["--baseline-rev".to_owned(), rev.to_owned()]);
        } else if let Some(v) = baseline(&p.name) {
            args.extend(["--baseline-version".to_owned(), v]);
        }
        eprintln!("==> cargo-semver-checks {}", args.join(" "));
        if run_inherit(tool.as_os_str(), &args.map_os(), root).is_err() {
            failed.push(p.name.clone());
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(fail(format!(
            "cargo-semver-checks refused {}: a breaking change needs a new major version \
             (docs/versioning.md)",
            failed.join(", ")
        )))
    }
}

/// The files of a crate's `src/` that gate code on the target, each with
/// the condition found.
fn target_gated(dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (path, bytes) in crate::fsx::read_tree(dir, "src")? {
        let text = String::from_utf8_lossy(&bytes);
        for cfg in ["target_arch", "target_family", "target_os"] {
            if text.contains(cfg) {
                out.push(format!("{path}: {cfg}"));
            }
        }
    }
    Ok(out)
}

/// The pinned `cargo-semver-checks`, installed into `target/tools` by cargo
/// when it is missing or another version.
fn install_semver_checks(root: &Path) -> Result<std::path::PathBuf> {
    let tools = root.join("target").join("tools");
    let tool = tools.join("bin").join("cargo-semver-checks");
    let installed = run_capture(tool.as_os_str(), &["--version"].map_os(), root, &[])
        .map(|v| String::from_utf8_lossy(&v).trim().to_owned())
        .unwrap_or_default();
    if installed != format!("cargo-semver-checks {SEMVER_CHECKS}") {
        eprintln!("==> cargo install cargo-semver-checks {SEMVER_CHECKS} into target/tools");
        let root_arg = tools.to_string_lossy().into_owned();
        run_inherit(
            &cargo(),
            &[
                "install",
                "--root",
                &root_arg,
                "cargo-semver-checks",
                "--version",
                SEMVER_CHECKS,
                "--locked",
            ]
            .map_os(),
            root,
        )?;
    }
    Ok(tool)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Registered, name_problems, publish_args, released, retry_after, target_gated};
    use crate::fsx::repo_root;

    /// This tree's packages, as far as these tests need: `mf2-model`'s.
    fn local() -> BTreeMap<&'static str, String> {
        BTreeMap::from([("mf2-model", "aaaa".to_owned())])
    }

    #[test]
    fn the_host_web_api_is_not_gated_on_the_target() {
        let crates = repo_root().join("crates");
        assert!(
            target_gated(&crates.join("mf2-host-web"))
                .unwrap()
                .is_empty()
        );
        // The negative control: a crate that does gate code on the target.
        assert!(
            !target_gated(&crates.join("mf2-runtime"))
                .unwrap()
                .is_empty()
        );
    }

    fn taken(versions: &[&str], owners: &[&str]) -> Registered {
        Registered::Taken {
            versions: versions.iter().map(|v| (*v).to_owned()).collect(),
            owners: owners.iter().map(|o| (*o).to_owned()).collect(),
            this: None,
        }
    }

    /// Published at the version being released, with this checksum.
    fn out(version: &str, owners: &[&str], sum: &str) -> Registered {
        Registered::Taken {
            versions: vec![version.to_owned()],
            owners: owners.iter().map(|o| (*o).to_owned()).collect(),
            this: Some(sum.to_owned()),
        }
    }

    fn free(names: &[&'static str]) -> Vec<(&'static str, Registered)> {
        names.iter().map(|n| (*n, Registered::Free)).collect()
    }

    #[test]
    fn the_first_release_needs_every_name_free() {
        assert!(name_problems(&free(&["mf2", "mf2-model"]), "1.0.0", None, &local()).is_empty());
    }

    // Negative controls: a name taken by someone else, before and after
    // the first release, and a version already out.

    #[test]
    fn a_taken_name_is_refused_on_the_first_release() {
        let mut found = free(&["mf2"]);
        found.push(("mf2-model", taken(&["0.1.0"], &["someone"])));
        assert_eq!(
            name_problems(&found, "1.0.0", None, &local()),
            [
                "mf2-model: taken on crates.io (owners: someone), and nothing of ours is \
              published yet"
            ]
        );
    }

    #[test]
    fn a_later_release_accepts_ours_and_new_names() {
        let found = vec![
            ("mf2", taken(&["1.0.0"], &["owner"])),
            ("mf2-model", taken(&["1.0.0"], &["owner", "other"])),
            ("mf2-new", Registered::Free),
        ];
        assert!(name_problems(&found, "1.0.1", Some("1.0.0"), &local()).is_empty());
    }

    #[test]
    fn a_name_someone_else_owns_is_refused_later() {
        let found = vec![
            ("mf2", taken(&["1.0.0"], &["owner"])),
            ("mf2-new", taken(&["0.1.0"], &["someone"])),
        ];
        assert_eq!(
            name_problems(&found, "1.1.0", Some("1.0.0"), &local()),
            [
                "mf2-new: owned on crates.io by someone, none of whom owns a published crate from this project"
            ]
        );
    }

    #[test]
    fn a_continuation_after_the_partial_first_release_uses_its_owners() {
        let found = vec![
            ("mf2", Registered::Free),
            ("mf2-model", taken(&["1.0.0"], &["owner"])),
            ("mf2-resource", taken(&["1.0.0"], &["owner"])),
            ("mf2-new", Registered::Free),
        ];
        assert!(name_problems(&found, "1.0.1", Some("1.0.0"), &local()).is_empty());
    }

    #[test]
    fn a_facade_without_the_previous_release_is_refused() {
        let found = vec![("mf2", taken(&["0.1.0"], &["someone"]))];
        assert_eq!(
            name_problems(&found, "1.0.1", Some("1.0.0"), &local()),
            [
                "mf2: crates.io does not have 1.0.0, the release before this one in \
              CHANGELOG.md, so its owners are not known to be ours"
            ]
        );
        assert_eq!(
            name_problems(&free(&["mf2"]), "1.0.1", Some("1.0.0"), &local()),
            ["mf2: not on crates.io, though CHANGELOG.md says 1.0.0 was released"]
        );
    }

    #[test]
    fn a_publish_stopped_part_way_is_taken_up() {
        // The first release: `mf2-model` is out and identical, `mf2` free.
        let found = vec![
            ("mf2", Registered::Free),
            ("mf2-model", out("1.0.0", &["owner"], "aaaa")),
        ];
        assert!(name_problems(&found, "1.0.0", None, &local()).is_empty());
        assert_eq!(released(&found, &local()), ["mf2-model"]);
        assert_eq!(
            publish_args(&released(&found, &local())),
            ["publish", "--workspace", "--exclude", "mf2-model"]
        );
    }

    // Negative control: this version out, but not this tree's package.
    #[test]
    fn a_published_version_that_differs_is_refused() {
        let found = vec![("mf2-model", out("1.0.0", &["owner"], "bbbb"))];
        assert_eq!(
            name_problems(&found, "1.0.0", None, &local()),
            [
                "mf2-model: 1.0.0 is already on crates.io and is not this tree's package (a \
              published version cannot be replaced)"
            ]
        );
        assert!(released(&found, &local()).is_empty());
    }

    #[test]
    fn the_rate_limits_time_is_read() {
        // Cargo's words on 2026-09-26, from the owner's first publish.
        let said = "the remote server responded with an error (status 429 Too Many \
                    Requests): You have published too many new crates in a short period of \
                    time. Please try again after Sat, 26 Sep 2026 03:15:30 GMT and see \
                    https://crates.io/docs/rate-limits for more details.";
        // `date -u -d '2026-09-26 03:15:30' +%s`
        assert_eq!(retry_after(said), Some(1_790_392_530));
        assert_eq!(retry_after("try again later"), None);
    }
}
