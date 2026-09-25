//! `cargo xtask fluent-ab`: the `leptos-fluent` A/B, measured once as a
//! snapshot (`plans/16-phase-8-work-order.md` A5; `plans/06-size-and-perf.md`
//! §6; `bench/fluent-ab/README.md`).
//!
//! 1. The reference workload (default knobs, seed 1) with the `fluent-view`
//!    application — the reference application on `leptos-fluent`, in its
//!    own idiom — and the workload's `.ftl` files.
//! 2. **The mf2 side is that application migrated**, not a twin written by
//!    hand: a copy converted by `mf2 init --no-messages` and `mf2 convert
//!    --from leptos-fluent --write`, whose report must be exactly the
//!    hand-finishing the guide describes, then finished as the guide says
//!    (`bench/fluent-ab/mf2/`: the manifest, the entry points, the server;
//!    the shell's `<html lang dir>` and catalog links, edited in place).
//! 3. Both built for the client with the profile and `wasm-opt` flags of
//!    `cargo xtask size` — once as they ship, for the sizes, and once with
//!    `ab-bench`, the timing hooks, for the browser — and the server of
//!    each. A third client, `fluent-view` with only the source locale's
//!    `.ftl`, gives what each added locale costs on the Fluent side.
//! 4. Both servers run at once; `tools/e2e/checks/fluent-ab.mjs` first
//!    checks that the two applications show the same text on every route in
//!    two locales, then times them alternately, a fresh first visit each.
//! 5. The report, `target/fluent-ab/SNAPSHOT.md` and `snapshot.json`;
//!    `--snapshot` also writes them to `bench/fluent-ab/`, which is only
//!    allowed from a clean tree, since the snapshot names the commit.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::cmd::{cargo, run_capture, run_inherit, run_inherit_env};
use crate::error::{Error, Result};
use crate::{docs, fluent_migrate, fsx};

/// How many distinct simple messages the 2,000 live nodes cycle over.
const NODE_MESSAGES: usize = 50;
/// The two servers.
const FLUENT_ADDR: &str = "127.0.0.1:3811";
const MF2_ADDR: &str = "127.0.0.1:3812";

fn fail(message: impl Into<String>) -> Error {
    Error::FluentAb(message.into())
}

/// One application of the A/B.
struct App {
    /// Its directory under the workload.
    dir: &'static str,
    /// The cargo package, which is also the server binary.
    package: &'static str,
    /// The library, which names the wasm and its JS.
    lib: &'static str,
}

const FLUENT: App = App {
    dir: "app-fluent-view",
    package: "workload-app-fluent-view",
    lib: "workload_app_fluent_view",
};
const MF2: App = App {
    dir: "app-mf2",
    package: "workload-app-mf2",
    lib: "workload_app_mf2",
};
const FLUENT_EN: App = App {
    dir: "app-fluent-en",
    package: "workload-app-fluent-en",
    lib: "workload_app_fluent_en",
};

pub(crate) struct Options {
    pub(crate) build: bool,
    pub(crate) browser: String,
    pub(crate) runs: u32,
    pub(crate) snapshot: bool,
}

pub(crate) fn run(root: &Path, options: &Options) -> Result<()> {
    let out = root.join("target/fluent-ab");
    let wl = out.join("wl");
    if options.snapshot {
        let dirty = run_capture(
            OsStr::new("git"),
            &[
                OsStr::new("status"),
                OsStr::new("--porcelain"),
                OsStr::new("--"),
                OsStr::new("."),
                OsStr::new(":!bench/fluent-ab/SNAPSHOT.md"),
                OsStr::new(":!bench/fluent-ab/snapshot.json"),
            ],
            root,
            &[],
        )?;
        if !dirty.is_empty() {
            return Err(fail(
                "--snapshot names the commit it measured, so the tree must be clean: \
                 commit first",
            ));
        }
    }
    if options.build {
        prepare(root, &out, &wl)?;
        build(root, &out, &wl)?;
    }
    let sizes = measure_sizes(&out, &wl)?;
    let browser = run_browser(root, &out, &wl, options)?;
    report(root, &out, &wl, &sizes, &browser, options)
}

// ---------------------------------------------------------------------------
// 1–2. The workload, the migration, the hooks.

fn prepare(root: &Path, out: &Path, wl: &Path) -> Result<()> {
    fsx::remove(wl)?;
    let view = root.join("bench/workload-gen/templates/fluent-view");
    eprintln!("==> workload-gen all -t fluent-view --format ftl");
    run_inherit(
        &cargo(),
        &[
            OsStr::new("run"),
            OsStr::new("--release"),
            OsStr::new("--quiet"),
            OsStr::new("-p"),
            OsStr::new("workload-gen"),
            OsStr::new("--"),
            OsStr::new("all"),
            OsStr::new("-t"),
            view.as_os_str(),
            OsStr::new("--format"),
            OsStr::new("ftl"),
            OsStr::new("--out"),
            wl.as_os_str(),
        ],
        root,
    )?;
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
    let ids = Ids::choose(wl)?;
    let fluent = wl.join(FLUENT.dir);
    let mf2 = wl.join(MF2.dir);
    copy_tree(&fluent, &mf2)?;
    migrate(root, &mf2)?;
    finish_mf2(root, &mf2, &ids)?;
    hook_fluent(root, &fluent, &ids)?;
    source_locale_only(wl)?;
    fsx::write(&out.join("ids.json"), ids.to_json().as_bytes())
}

/// `mf2 init --no-messages`, then `mf2 convert --from leptos-fluent --write`:
/// the guide's commands, whose report must be the hand-finishing only.
fn migrate(root: &Path, app: &Path) -> Result<()> {
    let mf2 = root.join("target/debug/mf2");
    let i18n = app.join("i18n");
    eprintln!("==> mf2 -C i18n init --name workload-i18n --no-messages");
    run_inherit(
        mf2.as_os_str(),
        &[
            OsStr::new("-C"),
            i18n.as_os_str(),
            OsStr::new("init"),
            OsStr::new("--name"),
            OsStr::new("workload-i18n"),
            OsStr::new("--no-messages"),
            OsStr::new("--locale"),
            OsStr::new("pl"),
            OsStr::new("--locale"),
            OsStr::new("en-XA"),
            OsStr::new("--locale"),
            OsStr::new("ar-XB"),
        ],
        root,
    )?;
    // Our crates are not published yet: path dependencies on the tree, as
    // `cargo xtask docs` makes them.
    let manifest = i18n.join("Cargo.toml");
    let text = fsx::read_to_string(&manifest)?;
    let text = docs::local_dependencies(root, &manifest, &text)?;
    fsx::write(&manifest, text.as_bytes())?;

    eprintln!("==> mf2 -C i18n convert --from leptos-fluent . --write");
    let output = Command::new(&mf2)
        .arg("-C")
        .arg(&i18n)
        .args(["convert", "--from", "leptos-fluent"])
        .arg(app)
        .args(["--write", "--format", "json"])
        .output()
        .map_err(|source| Error::Spawn {
            program: "mf2".to_owned(),
            source,
        })?;
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        fail(format!(
            "the report is not JSON ({e}): {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    })?;
    fluent_migrate::check_report(&report, app).map_err(|e| fail(e.to_string()))
}

/// The guide's hand-finishing: the manifest, the entry points and the
/// server from `bench/fluent-ab/mf2/`, and the shell edited in place.
fn finish_mf2(root: &Path, app: &Path, ids: &Ids) -> Result<()> {
    let finish = root.join("bench/fluent-ab/mf2");
    let manifest = fsx::read_to_string(&finish.join("Cargo.toml"))?
        .replace("{{ROOT}}", &root.display().to_string());
    fsx::write(&app.join("Cargo.toml"), manifest.as_bytes())?;
    for file in ["src/lib.rs", "src/main.rs", "src/support.rs"] {
        let text = fsx::read_to_string(&finish.join(file))?;
        fsx::write(&app.join(file), text.as_bytes())?;
    }
    let ab = fsx::read_to_string(&finish.join("src/ab.rs"))?;
    fsx::write(
        &app.join("src/ab.rs"),
        ids.fill(
            &ab,
            "        {i} => view! {{ <span>{{tr!(\"{id}\")}}</span> }}.into_any(),",
        )
        .as_bytes(),
    )?;

    // The shell (guide, "What is left to do by hand"): the provider goes, and
    // the document gains `<html lang dir>` and the catalog links.
    let path = app.join("src/app.rs");
    let mut text = fsx::read_to_string(&path)?;
    for (from, to) in [
        ("use crate::support::I18nProvider;\n\n", ""),
        (
            "use crate::components::*;\n",
            "use crate::components::*;\nuse leptos_mf2::{CatalogLinks, CatalogPreload, html_lang};\n",
        ),
        ("        <I18nProvider>\n", ""),
        ("        </I18nProvider>\n", ""),
        (
            "pub fn shell(options: LeptosOptions) -> impl IntoView {\n    view! {\n",
            "pub fn shell(options: LeptosOptions) -> impl IntoView {\n    let (lang, dir) = html_lang();\n    view! {\n",
        ),
        (
            "/pkg/workload-app-fluent-view.css",
            "/pkg/workload-app-mf2.css",
        ),
        ("<html lang=\"en\" dir=\"ltr\">", "<html lang=lang dir=dir>"),
        (
            "                <HydrationScripts options/>\n",
            "                <HydrationScripts options/>\n                <CatalogPreload/>\n                <CatalogLinks/>\n",
        ),
    ] {
        text = replace_once(&path, &text, from, to)?;
    }
    fsx::write(&path, text.as_bytes())
}

/// `fluent-view` as generated, plus the timing hooks behind `ab-bench` (which
/// the size build leaves off).
fn hook_fluent(root: &Path, app: &Path, ids: &Ids) -> Result<()> {
    let ab = fsx::read_to_string(&root.join("bench/fluent-ab/fluent/ab.rs"))?;
    fsx::write(
        &app.join("src/ab.rs"),
        ids.fill(
            &ab,
            "        {i} => view! {{ <span>{{move_tr!(\"{id}\")}}</span> }}.into_any(),",
        )
        .as_bytes(),
    )?;
    let path = app.join("src/lib.rs");
    let mut text = fsx::read_to_string(&path)?;
    for (from, to) in [
        (
            "pub mod widgets;\n",
            "pub mod widgets;\n\n/// The A/B's timing hooks, in the timed build only.\n\
             #[cfg(all(feature = \"ab-bench\", feature = \"hydrate\"))]\npub mod ab;\n",
        ),
        (
            "    leptos::mount::hydrate_lazy(app::App);\n",
            "    #[cfg(not(feature = \"ab-bench\"))]\n    leptos::mount::hydrate_lazy(app::App);\n\
             \x20   #[cfg(feature = \"ab-bench\")]\n    leptos::mount::hydrate_lazy(ab::app);\n",
        ),
    ] {
        text = replace_once(&path, &text, from, to)?;
    }
    fsx::write(&path, text.as_bytes())?;
    let path = app.join("Cargo.toml");
    let text = replace_once(
        &path,
        &fsx::read_to_string(&path)?,
        "[features]\n",
        "[features]\n# The A/B's timing hooks (src/ab.rs), in the timed build only.\nab-bench = []\n",
    )?;
    fsx::write(&path, text.as_bytes())
}

/// `fluent-view` with the source locale's `.ftl` only: the Fluent side
/// embeds every locale, so the difference is what the others cost.
fn source_locale_only(wl: &Path) -> Result<()> {
    let app = wl.join(FLUENT_EN.dir);
    copy_tree(&wl.join(FLUENT.dir), &app)?;
    copy_tree(&wl.join("ftl/en"), &wl.join("ftl-en/en"))?;
    let path = app.join("Cargo.toml");
    let text = fsx::read_to_string(&path)?.replace(FLUENT.package, FLUENT_EN.package);
    fsx::write(&path, text.as_bytes())?;
    let path = app.join("src/support.rs");
    let text = replace_once(
        &path,
        &fsx::read_to_string(&path)?,
        "locales: \"../ftl\",",
        "locales: \"../ftl-en\",",
    )?;
    fsx::write(&path, text.as_bytes())
}

fn replace_once(path: &Path, text: &str, from: &str, to: &str) -> Result<String> {
    match text.matches(from).count() {
        1 => Ok(text.replacen(from, to, 1)),
        n => Err(fail(format!(
            "{}: expected {from:?} once, found it {n} times",
            path.display()
        ))),
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fsx::remove(to)?;
    for (path, bytes) in fsx::read_tree(from, ".")? {
        fsx::write(&to.join(path.trim_start_matches("./")), &bytes)?;
    }
    Ok(())
}

/// The messages the timing hooks use, chosen from the workload so that the
/// same ones are formatted on both sides.
struct Ids {
    /// Simple messages whose `pl` text differs from the `en` text, so that a
    /// switch changes every node.
    nodes: Vec<String>,
    simple: String,
    one: (String, String),
    select: (String, String),
}

impl Ids {
    fn choose(wl: &Path) -> Result<Ids> {
        let read = |tag: &str| -> Result<BTreeMap<String, String>> {
            let path = wl.join(format!("json/{tag}.json"));
            serde_json::from_str(&fsx::read_to_string(&path)?)
                .map_err(|e| fail(format!("{}: {e}", path.display())))
        };
        let en = read("en")?;
        let pl = read("pl")?;
        let fluent_id = |id: &str| id.replace('.', "-");
        let simple = |source: &str| !source.contains('{') && !source.starts_with('.');
        let nodes: Vec<String> = en
            .iter()
            .filter(|(id, source)| {
                simple(source)
                    && pl.get(*id).is_some_and(|p| p != *source)
                    && !id.contains("canary")
            })
            .map(|(id, _)| fluent_id(id))
            .take(NODE_MESSAGES)
            .collect();
        if nodes.len() < NODE_MESSAGES {
            return Err(fail("the workload has too few simple messages"));
        }
        // One placeholder, a variable, and nothing else in braces.
        let one = en
            .iter()
            .filter(|(id, _)| !id.contains("canary"))
            .find_map(|(id, source)| {
                if source.starts_with('.') || source.matches('{').count() != 1 {
                    return None;
                }
                let start = source.find("{$")? + 2;
                let len = source.get(start..)?.find('}')?;
                let name = source.get(start..start + len)?;
                name.bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'_')
                    .then(|| (fluent_id(id), name.to_owned()))
            })
            .ok_or_else(|| fail("the workload has no one-argument message"))?;
        let select = en
            .iter()
            .find_map(|(id, source)| {
                let rest = source.strip_prefix(".input {$")?;
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || *c == '_')
                    .collect();
                (!name.is_empty()).then(|| (fluent_id(id), name))
            })
            .ok_or_else(|| fail("the workload has no select"))?;
        Ok(Ids {
            simple: nodes.first().cloned().unwrap_or_default(),
            nodes,
            one,
            select,
        })
    }

    /// `template` with the placeholders filled; `arm` is one match arm, with
    /// `{i}` and `{id}`.
    fn fill(&self, template: &str, arm: &str) -> String {
        let arms: Vec<String> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let i = if i + 1 == self.nodes.len() {
                    "_".to_owned()
                } else {
                    i.to_string()
                };
                arm.replace("{i}", &i)
                    .replace("{id}", id)
                    .replace("{{", "{")
                    .replace("}}", "}")
            })
            .collect();
        template
            .replace("{{NODE_COUNT}}", &self.nodes.len().to_string())
            .replace("{{NODE_ARMS}}", &arms.join("\n"))
            .replace("{{SIMPLE}}", &self.simple)
            .replace("{{ONE}}", &self.one.0)
            .replace("{{ONE_ARG}}", &self.one.1)
            .replace("{{SELECT}}", &self.select.0)
            .replace("{{SELECT_ARG}}", &self.select.1)
    }

    fn to_json(&self) -> String {
        serde_json::json!({
            "nodes": self.nodes,
            "simple": self.simple,
            "one": { "id": self.one.0, "argument": self.one.1 },
            "select": { "id": self.select.0, "argument": self.select.1 },
        })
        .to_string()
    }
}

// ---------------------------------------------------------------------------
// 3. The builds.

fn build(root: &Path, out: &Path, wl: &Path) -> Result<()> {
    let target = out.join("target");
    let jobs = std::env::var_os("CARGO_BUILD_JOBS").unwrap_or_else(|| OsString::from("3"));
    let envs: [(&str, &OsStr); 2] = [
        ("CARGO_TARGET_DIR", target.as_os_str()),
        ("CARGO_BUILD_JOBS", jobs.as_os_str()),
    ];
    // The sizes: each client as `cargo xtask size` builds it.
    for app in [&FLUENT, &MF2, &FLUENT_EN] {
        eprintln!("==> {}: client, as `cargo xtask size` builds it", app.dir);
        run_inherit_env(
            &cargo(),
            &[
                OsStr::new("build"),
                OsStr::new("--quiet"),
                OsStr::new("--lib"),
                OsStr::new("--no-default-features"),
                OsStr::new("--features"),
                OsStr::new("hydrate"),
                OsStr::new("--target"),
                OsStr::new("wasm32-unknown-unknown"),
                OsStr::new("--profile"),
                OsStr::new("wasm-release"),
            ],
            &wl.join(app.dir),
            &envs,
        )?;
        let wasm = target.join(format!(
            "wasm32-unknown-unknown/wasm-release/{}.wasm",
            app.lib
        ));
        let pkg = out.join(format!("pkg-size-{}", app.dir));
        crate::b5::ship(root, &wasm, &pkg, app.lib)?;
    }
    // The site the browser times: the lazy routes are `wasm_split` imports,
    // which only `cargo leptos --split` resolves, so the served application
    // is built the way the generated applications ship — with the hooks.
    for app in [&FLUENT, &MF2] {
        eprintln!(
            "==> {}: cargo leptos build --release --split, hydrate + ab-bench",
            app.dir
        );
        run_inherit_env(
            OsStr::new("cargo"),
            &[
                OsStr::new("leptos"),
                OsStr::new("build"),
                OsStr::new("--release"),
                OsStr::new("--split"),
                OsStr::new("--lib-features"),
                OsStr::new("hydrate,ab-bench"),
            ],
            &wl.join(app.dir),
            &envs,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Sizes.

/// Raw, `gzip -9` and brotli (quality 11, window 22 — what `mf2-axum` serves
/// a catalog with, and B7's measure) of one file.
#[derive(Clone, Copy, Default)]
struct Size {
    raw: u64,
    gz: u64,
    br: u64,
}

impl Size {
    fn of(bytes: &[u8]) -> Result<Size> {
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(9));
        std::io::Write::write_all(&mut gz, bytes)?;
        let gz = gz.finish()?;
        let mut br = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut br, 4096, 11, 22);
            std::io::Write::write_all(&mut writer, bytes)?;
        }
        Ok(Size {
            raw: bytes.len() as u64,
            gz: gz.len() as u64,
            br: br.len() as u64,
        })
    }

    fn file(path: &Path) -> Result<Size> {
        let bytes = std::fs::read(path).map_err(|source| Error::IoAt {
            path: path.to_path_buf(),
            source,
        })?;
        Size::of(&bytes)
    }

    fn plus(self, other: Size) -> Size {
        Size {
            raw: self.raw + other.raw,
            gz: self.gz + other.gz,
            br: self.br + other.br,
        }
    }

    fn json(self) -> serde_json::Value {
        serde_json::json!({ "raw": self.raw, "gz": self.gz, "br": self.br })
    }
}

struct Sizes {
    /// `(wasm, js)` of each shipped client.
    fluent: (Size, Size),
    fluent_en: (Size, Size),
    mf2: (Size, Size),
    /// Each locale's catalog, as `mf2-axum` serves it.
    catalogs: Vec<(String, Size)>,
}

fn measure_sizes(out: &Path, wl: &Path) -> Result<Sizes> {
    let client = |app: &App| -> Result<(Size, Size)> {
        let pkg = out.join(format!("pkg-size-{}", app.dir));
        Ok((
            Size::file(&pkg.join("opt.wasm"))?,
            Size::file(&pkg.join(format!("{}.js", app.lib)))?,
        ))
    };
    // The catalogs as the i18n crate's build writes them (`mf2 compile`
    // runs the same build with the same `mf2.toml`).
    let catalogs_dir = out.join("catalogs");
    fsx::remove(&catalogs_dir)?;
    let i18n = wl.join(MF2.dir).join("i18n");
    run_inherit(
        fsx::repo_root().join("target/debug/mf2").as_os_str(),
        &[
            OsStr::new("-C"),
            i18n.as_os_str(),
            OsStr::new("compile"),
            OsStr::new("--site"),
            catalogs_dir.as_os_str(),
        ],
        &fsx::repo_root(),
    )?;
    let index: serde_json::Value =
        serde_json::from_str(&fsx::read_to_string(&catalogs_dir.join("index.json"))?)
            .map_err(|e| fail(format!("index.json: {e}")))?;
    let mut catalogs = Vec::new();
    for tag in ["en", "pl", "en-XA", "ar-XB"] {
        let file = catalog_file(&index, tag)
            .ok_or_else(|| fail(format!("index.json names no catalog for {tag}: {index}")))?;
        catalogs.push((tag.to_owned(), Size::file(&catalogs_dir.join(file))?));
    }
    Ok(Sizes {
        fluent: client(&FLUENT)?,
        fluent_en: client(&FLUENT_EN)?,
        mf2: client(&MF2)?,
        catalogs,
    })
}

/// The file `index.json` names for `tag`, whatever the index's exact shape:
/// the first string under the tag that looks like a catalog file.
fn catalog_file(index: &serde_json::Value, tag: &str) -> Option<String> {
    fn find(value: &serde_json::Value, tag: &str, under: bool) -> Option<String> {
        match value {
            serde_json::Value::String(s) if under && s.contains('.') => {
                Some(s.rsplit('/').next().unwrap_or(s).to_owned())
            }
            serde_json::Value::Object(map) => map.iter().find_map(|(k, v)| {
                find(
                    v,
                    tag,
                    under || k == tag || v.get("tag") == Some(&tag.into()),
                )
            }),
            serde_json::Value::Array(items) => items.iter().find_map(|v| {
                find(
                    v,
                    tag,
                    under
                        || v.get("locale") == Some(&tag.into())
                        || v.get("tag") == Some(&tag.into()),
                )
            }),
            _ => None,
        }
    }
    find(index, tag, false)
}

// ---------------------------------------------------------------------------
// 4. The browser.

/// A server that is killed when dropped.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn serve(out: &Path, wl: &Path, app: &App, addr: &str) -> Result<Server> {
    let binary = out.join("target/release").join(app.package);
    let site = wl.join(app.dir).join("target/site");
    let child = Command::new(&binary)
        .current_dir(wl.join(app.dir))
        .env("LEPTOS_OUTPUT_NAME", app.package)
        .env("LEPTOS_SITE_ROOT", &site)
        .env("LEPTOS_SITE_PKG_DIR", "pkg")
        .env("LEPTOS_SITE_ADDR", addr)
        .env("LEPTOS_ENV", "PROD")
        .stdout(Stdio::null())
        .spawn()
        .map_err(|source| Error::Spawn {
            program: binary.display().to_string(),
            source,
        })?;
    let server = Server(child);
    let started = Instant::now();
    while TcpStream::connect(addr).is_err() {
        if started.elapsed() > Duration::from_secs(30) {
            return Err(fail(format!("{} did not listen on {addr}", app.package)));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(server)
}

fn run_browser(root: &Path, out: &Path, wl: &Path, options: &Options) -> Result<serde_json::Value> {
    let _fluent = serve(out, wl, &FLUENT, FLUENT_ADDR)?;
    let _mf2 = serve(out, wl, &MF2, MF2_ADDR)?;
    let json = out.join("browser.json");
    let before = load_average();
    eprintln!("==> tools/e2e/checks/fluent-ab.mjs in {}", options.browser);
    let fluent_url = format!("http://{FLUENT_ADDR}");
    let mf2_url = format!("http://{MF2_ADDR}");
    let runs = options.runs.to_string();
    let ids = out.join("ids.json");
    let fluent_js = format!("/pkg/{}.js", FLUENT.package);
    let mf2_js = format!("/pkg/{}.js", MF2.package);
    let result = run_inherit_env(
        OsStr::new("node"),
        &[
            OsStr::new("run.mjs"),
            OsStr::new("fluent-ab"),
            OsStr::new("--browser"),
            OsStr::new(&options.browser),
            OsStr::new("--json"),
            json.as_os_str(),
        ],
        &root.join("tools/e2e"),
        &[
            ("FLUENT_AB_FLUENT_URL", OsStr::new(&fluent_url)),
            ("FLUENT_AB_MF2_URL", OsStr::new(&mf2_url)),
            ("FLUENT_AB_FLUENT_JS", OsStr::new(&fluent_js)),
            ("FLUENT_AB_MF2_JS", OsStr::new(&mf2_js)),
            ("FLUENT_AB_RUNS", OsStr::new(&runs)),
            ("FLUENT_AB_IDS", ids.as_os_str()),
        ],
    );
    let after = load_average();
    result?;
    let mut value: serde_json::Value = serde_json::from_str(&fsx::read_to_string(&json)?)
        .map_err(|e| fail(format!("browser.json: {e}")))?;
    value["load"] = serde_json::json!({ "before": before, "after": after });
    Ok(value)
}

/// `/proc/loadavg`'s three averages.
fn load_average() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// 5. The report.

fn version_of(lock: &Path, package: &str) -> String {
    let Ok(text) = std::fs::read_to_string(lock) else {
        return String::new();
    };
    let Ok(lock) = text.parse::<toml::Table>() else {
        return String::new();
    };
    let mut versions: Vec<String> = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|p| p.get("name").and_then(toml::Value::as_str) == Some(package))
        .filter_map(|p| {
            p.get("version")
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    versions.sort();
    versions.dedup();
    versions.join(", ")
}

/// The version of `package` in the client build of `app`, as `cargo tree`
/// resolves it (the lock file also names lines that are not built).
fn built_version(out: &Path, app: &Path, package: &str) -> String {
    let target = out.join("target");
    let tree = run_capture(
        &cargo(),
        &[
            OsStr::new("tree"),
            OsStr::new("-q"),
            OsStr::new("--no-default-features"),
            OsStr::new("--features"),
            OsStr::new("hydrate"),
            OsStr::new("--target"),
            OsStr::new("wasm32-unknown-unknown"),
            OsStr::new("--prefix"),
            OsStr::new("none"),
            OsStr::new("-e"),
            OsStr::new("normal"),
        ],
        app,
        &[("CARGO_TARGET_DIR", &target.display().to_string())],
    )
    .unwrap_or_default();
    let prefix = format!("{package} v");
    let mut versions: Vec<String> = String::from_utf8_lossy(&tree)
        .lines()
        .filter_map(|l| l.strip_prefix(&prefix))
        .map(|v| v.split_whitespace().next().unwrap_or("").to_owned())
        .collect();
    versions.sort();
    versions.dedup();
    versions.join(", ")
}

fn tool_version(program: &str) -> String {
    run_capture(
        OsStr::new(program),
        &[OsStr::new("--version")],
        &fsx::repo_root(),
        &[],
    )
    .map(|v| String::from_utf8_lossy(&v).trim().to_owned())
    .unwrap_or_default()
}

fn report(
    root: &Path,
    out: &Path,
    wl: &Path,
    sizes: &Sizes,
    browser: &serde_json::Value,
    options: &Options,
) -> Result<()> {
    let commit = run_capture(
        OsStr::new("git"),
        &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        root,
        &[],
    )
    .map(|v| String::from_utf8_lossy(&v).trim().to_owned())
    .unwrap_or_default();
    let date = run_capture(
        OsStr::new("date"),
        &[OsStr::new("-u"), OsStr::new("+%Y-%m-%dT%H:%MZ")],
        root,
        &[],
    )
    .map(|v| String::from_utf8_lossy(&v).trim().to_owned())
    .unwrap_or_default();
    let fluent_lock = wl.join(FLUENT.dir).join("Cargo.lock");
    let versions = serde_json::json!({
        "leptos-fluent": version_of(&fluent_lock, "leptos-fluent"),
        "fluent-bundle": version_of(&fluent_lock, "fluent-bundle"),
        "fluent-templates": version_of(&fluent_lock, "fluent-templates"),
        "leptos (leptos-fluent side)": built_version(out, &wl.join(FLUENT.dir), "leptos"),
        "leptos (mf2 side)": built_version(out, &wl.join(MF2.dir), "leptos"),
        "tachys (both sides)": built_version(out, &wl.join(MF2.dir), "tachys"),
        "rustc": tool_version("rustc"),
        "wasm-bindgen": tool_version("wasm-bindgen"),
        "wasm-opt": tool_version("wasm-opt"),
        "browsers": browser["runs"].as_array().into_iter().flatten()
            .map(|r| format!("{} {}", r["browser"].as_str().unwrap_or(""), r["version"].as_str().unwrap_or("")))
            .collect::<Vec<_>>(),
    });
    let sites = std::fs::read_to_string(wl.join("sites.json")).unwrap_or_default();
    let site_count = sites.lines().filter(|l| l.contains("\"site\"")).count();
    let rich_sites = sites
        .lines()
        .filter(|l| l.contains("\"mode\": \"rich\""))
        .count();

    let catalog = |tag: &str| {
        sizes
            .catalogs
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, s)| *s)
            .unwrap_or_default()
    };
    let fluent_first = sizes.fluent.0.plus(sizes.fluent.1);
    let mf2_first = sizes.mf2.0.plus(sizes.mf2.1).plus(catalog("en"));
    let fluent_locales = sizes.fluent.0.plus(sizes.fluent.1);
    let fluent_one = sizes.fluent_en.0.plus(sizes.fluent_en.1);
    // Three locales beyond the source in the full build.
    let per_locale = |f: fn(Size) -> u64| {
        let delta = f(fluent_locales).saturating_sub(f(fluent_one));
        delta.div_ceil(3)
    };

    let mut md = String::new();
    let _ = writeln!(md, "# `leptos-fluent` A/B — snapshot\n");
    let _ = writeln!(
        md,
        "Measured once, at migration (`plans/16-phase-8-work-order.md` A5; \
         `plans/06-size-and-perf.md` §6). Not re-run per commit; re-run only \
         when the owner asks, with the command below at that commit.\n"
    );
    let _ = writeln!(md, "| | |\n|---|---|");
    let _ = writeln!(md, "| commit measured | `{commit}` |");
    let _ = writeln!(md, "| date | {date} |");
    let _ = writeln!(
        md,
        "| command | `cargo xtask fluent-ab --browser {} --runs {}` |",
        options.browser, options.runs
    );
    let _ = writeln!(
        md,
        "| machine load (1/5/15 min), before / after the browser runs | {} / {} |",
        browser["load"]["before"].as_str().unwrap_or(""),
        browser["load"]["after"].as_str().unwrap_or("")
    );
    for (k, v) in versions.as_object().into_iter().flatten() {
        let v = match v {
            serde_json::Value::Array(items) => items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join("; "),
            other => other.as_str().unwrap_or("").to_owned(),
        };
        let _ = writeln!(md, "| {k} | {v} |");
    }
    let _ = writeln!(
        md,
        "\nThe application: the reference workload (seed 1, {site_count} call sites, \
         1,600 messages in `en`, `pl`, `en-XA`, `ar-XB`), template `fluent-view` on \
         `leptos-fluent`, and the same application migrated by `mf2 convert --from \
         leptos-fluent` and finished as the migration guide says. Like with like: \
         {rich_sites} sites show a sentence with an inline element, written on both \
         sides as the sentence split around the element (three messages each).\n"
    );

    let _ = writeln!(md, "## Size\n");
    let _ = writeln!(
        md,
        "The client as it ships: `wasm32-unknown-unknown`, profile `wasm-release`, \
         `wasm-bindgen --target web`, `wasm-opt -Oz` (the flags of `cargo xtask \
         size`). Bytes; gz is `gzip -9`, br is brotli quality 11, window 22.\n"
    );
    let _ = writeln!(
        md,
        "| | leptos-fluent raw | gz | br | mf2 raw | gz | br |\n|---|---:|---:|---:|---:|---:|---:|"
    );
    let row = |md: &mut String, name: &str, a: Size, b: Size| {
        let _ = writeln!(
            md,
            "| {name} | {} | {} | {} | {} | {} | {} |",
            a.raw, a.gz, a.br, b.raw, b.gz, b.br
        );
    };
    row(&mut md, "wasm", sizes.fluent.0, sizes.mf2.0);
    row(&mut md, "JS", sizes.fluent.1, sizes.mf2.1);
    row(&mut md, "the `en` catalog", Size::default(), catalog("en"));
    row(
        &mut md,
        "**a first visit in `en`** (wasm + JS + that locale's text)",
        fluent_first,
        mf2_first,
    );
    let _ = writeln!(
        md,
        "\nOn `leptos-fluent` every locale's text is in the wasm, so every visitor \
         downloads all four. **What each added locale costs** — on `leptos-fluent` \
         every visitor pays it, measured as the client with all four locales less the \
         client with `en` only ({} / {} / {} B raw / gz / br for `en` only), over the \
         three: **{} B raw, {} B gz, {} B br per locale**. On mf2 the wasm does not \
         change; only a reader of that locale downloads its catalog:\n",
        fluent_one.raw,
        fluent_one.gz,
        fluent_one.br,
        per_locale(|s| s.raw),
        per_locale(|s| s.gz),
        per_locale(|s| s.br),
    );
    let _ = writeln!(md, "| catalog | raw | gz | br |\n|---|---:|---:|---:|");
    let served = |run: &serde_json::Value, side: &str, key: &str| {
        run["data"][format!("{side}Transfer")][key]
            .as_u64()
            .unwrap_or(0)
    };
    let mut served_md = String::new();
    for run in browser["runs"].as_array().into_iter().flatten() {
        let _ = writeln!(
            served_md,
            "| {} | {} | {} | {} |",
            run["browser"].as_str().unwrap_or(""),
            served(run, "fluent", "wasmBytes"),
            served(run, "mf2", "wasmBytes"),
            served(run, "mf2", "catalogBytes"),
        );
    }
    for (tag, s) in &sizes.catalogs {
        let _ = writeln!(md, "| `{tag}` | {} | {} | {} |", s.raw, s.gz, s.br);
    }

    let _ = writeln!(
        md,
        "\nThe sizes above are the whole client, every route in one module, as the \
         size gate builds it. The application the browser times is built the way the \
         generated applications ship, `cargo leptos build --release --split` (the \
         lazy routes are separate chunks, fetched when first visited), with the \
         `ab-bench` hooks; on a first visit to `/` the browser downloaded, as the test \
         servers send them (uncompressed; the catalog is brotli, as `mf2-axum` serves \
         it):\n\n| engine | leptos-fluent main wasm | mf2 main wasm | mf2 `en` catalog |\n\
         |---|---:|---:|---:|\n{served_md}"
    );
    let _ = writeln!(md, "\n## Speed\n");
    let _ = writeln!(
        md,
        "Each run is a fresh first visit (a new browser context, so an empty cache); \
         the two applications are run alternately, the order swapped every run, with \
         both servers up. Medians over the runs, milliseconds, with the minimum and \
         maximum. The timed build is the shipped one plus `ab-bench`'s hooks \
         (`bench/fluent-ab/README.md`).\n"
    );
    for run in browser["runs"].as_array().into_iter().flatten() {
        let name = run["browser"].as_str().unwrap_or("");
        let _ = writeln!(md, "### {name} {}\n", run["version"].as_str().unwrap_or(""));
        let _ = writeln!(md, "| measure | leptos-fluent | mf2 |\n|---|---:|---:|");
        let timings = &run["data"]["timings"];
        for (key, label) in [
            (
                "hydratedFromWasm",
                "first translated frame after the wasm's load (hydrated)",
            ),
            (
                "hydratedFromNavigation",
                "…the same, from the navigation's start",
            ),
            ("format0", "a simple message, µs per format"),
            ("format1", "a one-argument message, µs per format"),
            ("format2", "a plural select, µs per format"),
            ("mount", "mount 2,000 live translated nodes"),
            (
                "switch",
                "switch `en` → `pl` with 2,000 live nodes, until every node shows it",
            ),
            ("switchBack", "switch `pl` → `en`, the same"),
        ] {
            let cell = |side: &str| {
                let s = &timings[side][key];
                match (s["median"].as_f64(), s["min"].as_f64(), s["max"].as_f64()) {
                    (Some(m), Some(lo), Some(hi)) => format!("{m:.3} ({lo:.3}–{hi:.3})"),
                    _ => "—".to_owned(),
                }
            };
            let _ = writeln!(md, "| {label} | {} | {} |", cell("fluent"), cell("mf2"));
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "## The same text\n");
    for run in browser["runs"].as_array().into_iter().flatten() {
        let same = &run["data"]["sameText"];
        let _ = writeln!(
            md,
            "{}: {} — every route (`/`, `/r1`, `/r2`, `/r3`) in `en` and `pl`, the \
             hydrated `<main>`'s text compared whole ({} characters), bidi isolation \
             marks aside (an approved difference, owner question 5).\n",
            run["browser"].as_str().unwrap_or(""),
            if same["equal"].as_bool() == Some(true) {
                "equal"
            } else {
                "**different**"
            },
            same["characters"].as_u64().unwrap_or(0),
        );
    }

    let json = serde_json::json!({
        "commit": commit,
        "date": date,
        "command": format!("cargo xtask fluent-ab --browser {} --runs {}", options.browser, options.runs),
        "versions": versions,
        "load": browser["load"],
        "workload": { "sites": site_count, "richSites": rich_sites, "seed": 1 },
        "size": {
            "fluent": { "wasm": sizes.fluent.0.json(), "js": sizes.fluent.1.json(), "firstVisit": fluent_first.json(),
                        "sourceLocaleOnly": fluent_one.json(),
                        "perAddedLocale": { "raw": per_locale(|s| s.raw), "gz": per_locale(|s| s.gz), "br": per_locale(|s| s.br) } },
            "mf2": { "wasm": sizes.mf2.0.json(), "js": sizes.mf2.1.json(), "firstVisit": mf2_first.json(),
                     "catalogs": sizes.catalogs.iter().map(|(t, s)| (t.clone(), s.json())).collect::<serde_json::Map<_, _>>() },
        },
        "browser": browser["runs"].as_array().into_iter().flatten().map(|r| serde_json::json!({
            "browser": r["browser"], "version": r["version"], "data": r["data"],
            "assertions": r["assertions"],
        })).collect::<Vec<_>>(),
    });
    let json = serde_json::to_string_pretty(&json).map_err(|e| fail(e.to_string()))? + "\n";
    print!("{md}");
    fsx::write(&out.join("SNAPSHOT.md"), md.as_bytes())?;
    fsx::write(&out.join("snapshot.json"), json.as_bytes())?;
    if options.snapshot {
        let dir = root.join("bench/fluent-ab");
        fsx::write(&dir.join("SNAPSHOT.md"), md.as_bytes())?;
        fsx::write(&dir.join("snapshot.json"), json.as_bytes())?;
        eprintln!("==> fluent-ab: the snapshot is in bench/fluent-ab/ (commit {commit})");
    }
    Ok(())
}
