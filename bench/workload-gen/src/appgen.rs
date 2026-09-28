//! The generated cargo-leptos application (SSR + hydrate, Axum server,
//! `#[lazy_route]` routes, a `wasm-release` profile for the client library).

use std::fmt::Write as _;

use crate::error::Error;
use crate::model::{Workload, display_source};
use crate::output::Files;
use crate::rng::Rng;
use crate::sites::{Shape, SitePlan};
use crate::template::{ArgCtx, SiteCtx, Template};

/// Dependency versions of the generated app (the newest on 2026-09-24; Leptos
/// 0.9 is the default line, beta or not — D10).
pub mod versions {
    use crate::template::LeptosLine;

    /// `leptos`, `leptos_router` and `leptos_axum` on a Leptos line.
    pub const fn leptos(line: LeptosLine) -> (&'static str, &'static str, &'static str) {
        match line {
            LeptosLine::V0_9 => ("0.9.0-beta", "0.9.0-beta", "0.9.0-beta"),
            LeptosLine::V0_8 => ("0.8.20", "0.8.15", "0.8.10"),
        }
    }
    /// `axum`.
    pub const AXUM: &str = "0.8.9";
    /// `tokio`.
    pub const TOKIO: &str = "1.53.1";
    /// `wasm-bindgen` (must match the installed `wasm-bindgen` CLI).
    pub const WASM_BINDGEN: &str = "0.2.128";
}

/// Package name of a template's app: `workload-app-<template>`. Distinct per
/// template so that apps can share a `CARGO_TARGET_DIR` without cargo
/// mistaking one for another (same name + same workspace-relative paths
/// would look fresh).
pub fn crate_name(template: &Template) -> String {
    let suffix: String = template
        .name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("workload-app-{suffix}")
}

/// Elements per `<div class="block">`.
const BLOCK: usize = 8;

/// Escapes text for a Rust string literal.
pub fn rust_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out
}

fn site_ctx(wl: &Workload, plan: &SitePlan, i: usize) -> SiteCtx {
    let site = &plan.sites[i];
    let message = &wl.messages[site.message];
    let mut args: Vec<ArgCtx> = if site.mode.has_args() {
        message
            .vars
            .iter()
            .map(|v| ArgCtx {
                name: v.name,
                slot: 0,
                kind: v.kind,
            })
            .collect()
    } else {
        Vec::new()
    };
    args.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    for (slot, arg) in args.iter_mut().enumerate() {
        arg.slot = slot;
    }
    SiteCtx {
        id: message.id.clone(),
        fluent_id: crate::fluent::id(&message.id),
        index: wl.index[site.message],
        text: rust_escape(&display_source(message)),
        args,
        site: i,
        markup: message.markup.join(","),
    }
}

/// Every file of the app crate, paths relative to the crate root.
pub fn app_files(wl: &Workload, plan: &SitePlan, template: &Template) -> Result<Files, Error> {
    let k = plan.components;
    let width = (k.max(1) - 1).to_string().len().max(3);
    let names: Vec<String> = (0..k).map(|c| format!("{c:0width$}")).collect();

    let mut per_component: Vec<Vec<usize>> = vec![Vec::new(); k];
    for (i, site) in plan.sites.iter().enumerate() {
        per_component[site.component].push(i);
    }

    let mut files = Files::new();
    let mut tables = String::new();
    let mut components = String::from("//! Generated components (one module each).\n\n");
    for (c, sites) in per_component.iter().enumerate() {
        let name = &names[c];
        let (code, table) = component(wl, plan, template, c, name, sites)?;
        files.insert(format!("src/components/c{name}.rs"), code);
        tables.push_str(&table);
        let _ = writeln!(components, "pub mod c{name};");
    }
    components.push('\n');
    for name in &names {
        let _ = writeln!(components, "pub use c{name}::C{name};");
    }

    let krate = crate_name(template);
    files.insert("Cargo.toml", cargo_toml(wl, template, &krate));
    files.insert("src/lib.rs", lib_rs(template));
    files.insert(
        "src/main.rs",
        MAIN_RS.replace("workload_app", &krate.replace('-', "_")),
    );
    files.insert("src/app.rs", app_rs(plan, &names, &krate, template));
    files.insert("src/widgets.rs", WIDGETS_RS);
    files.insert("src/support.rs", template.support.clone());
    files.insert("src/components.rs", components);
    files.insert("src/tables.rs", tables_rs(template, &tables));
    files.insert("style/main.css", CSS);
    Ok(files)
}

enum Group {
    Match(Vec<usize>),
    Return(usize),
    Handler(Vec<usize>),
}

/// One component module, plus its deferred-label table (Rust source for
/// `src/tables.rs`, empty if the component has no deferred sites).
fn component(
    wl: &Workload,
    plan: &SitePlan,
    template: &Template,
    c: usize,
    name: &str,
    sites: &[usize],
) -> Result<(String, String), Error> {
    let by = |shape: Shape| -> Vec<usize> {
        sites
            .iter()
            .copied()
            .filter(|&i| plan.sites[i].shape == shape)
            .collect()
    };
    let render = |i: usize| -> Result<String, Error> {
        let site = &plan.sites[i];
        template.render_site(site.shape, site.mode, &site_ctx(wl, plan, i))
    };

    // String sites: match arms in helper functions, function returns, and
    // error values set from event handlers.
    let strings = by(Shape::String);
    let mut rng = Rng::stream(wl.knobs.seed, "groups", c as u64);
    let mut groups: Vec<Group> = Vec::new();
    let mut at = 0;
    while at < strings.len() {
        let remaining = strings.len() - at;
        let roll = rng.below(100);
        let group = if roll < 45 && remaining >= 2 {
            let size = remaining.min(3);
            Group::Match(strings[at..at + size].to_vec())
        } else if roll < 65 {
            Group::Return(strings[at])
        } else {
            let size = rng.range(1, 3).min(remaining);
            Group::Handler(strings[at..at + size].to_vec())
        };
        at += match &group {
            Group::Match(v) | Group::Handler(v) => v.len(),
            Group::Return(_) => 1,
        };
        groups.push(group);
    }

    // Button labels come from child sites (at most half of them).
    let mut children = by(Shape::Child);
    let handlers = groups
        .iter()
        .filter(|g| matches!(g, Group::Handler(_)))
        .count();
    let labels_n = handlers.min(children.len() / 2);
    let mut labels: Vec<usize> = children.split_off(children.len() - labels_n);

    let mut helpers = String::new();
    let mut elements: Vec<(usize, String)> = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        let fname = format!("s{name}_{g}");
        match group {
            Group::Match(v) => {
                let _ = writeln!(
                    helpers,
                    "fn {fname}(n: i64, who: &'static str, when: &'static str) -> String {{\n    match n.rem_euclid({}) {{",
                    v.len()
                );
                for (arm, &i) in v.iter().enumerate() {
                    let pat = if arm + 1 == v.len() {
                        "_".to_owned()
                    } else {
                        arm.to_string()
                    };
                    let _ = writeln!(helpers, "        {pat} => {},", render(i)?);
                }
                helpers.push_str("    }\n}\n\n");
                elements.push((
                    v[0],
                    format!("<p>{{move || {fname}(count.get(), who, when)}}</p>"),
                ));
            }
            Group::Return(i) => {
                let _ = writeln!(
                    helpers,
                    "fn {fname}(n: i64, who: &'static str, when: &'static str) -> String {{\n    {}\n}}\n",
                    render(*i)?
                );
                elements.push((*i, format!("<p>{{{fname}(n, who, when)}}</p>")));
            }
            Group::Handler(v) => {
                let value = if v.len() == 1 {
                    render(v[0])?
                } else {
                    let mut m = format!("match count.get_untracked().rem_euclid({}) {{ ", v.len());
                    for (arm, &i) in v.iter().enumerate() {
                        let pat = if arm + 1 == v.len() {
                            "_".to_owned()
                        } else {
                            arm.to_string()
                        };
                        let _ = write!(m, "{pat} => {}, ", render(i)?);
                    }
                    m.push('}');
                    m
                };
                let label = match labels.pop() {
                    Some(l) => format!("{{{}}}", render(l)?),
                    None => "\"Apply\"".to_owned(),
                };
                elements.push((
                    v[0],
                    format!(
                        "<button type=\"button\" on:click=move |_| {{\n                    set_count.update(|c| *c += 1);\n                    set_flag.update(|f| *f = !*f);\n                    set_error.set({value});\n                }}>{label}</button>"
                    ),
                ));
            }
        }
    }

    for &i in &children {
        elements.push((i, format!("<p>{{{}}}</p>", render(i)?)));
    }
    for pair in by(Shape::Attr).chunks(2) {
        let code = match pair {
            [a, b] => format!(
                "<input type=\"text\" aria-label={{{}}} placeholder={{{}}}/>",
                render(*a)?,
                render(*b)?
            ),
            [a] => format!("<input type=\"text\" aria-label={{{}}}/>", render(*a)?),
            _ => unreachable!("chunks(2)"),
        };
        elements.push((pair[0], code));
    }
    for (shape, widget) in [
        (Shape::TextProp, "TextLabel"),
        (Shape::SignalProp, "SignalHint"),
        (Shape::StringProp, "StringBadge"),
    ] {
        for i in by(shape) {
            elements.push((i, format!("<{widget} text={{{}}}/>", render(i)?)));
        }
    }
    for i in by(Shape::IfElse) {
        let Some(partner) = plan.sites[i].partner else {
            continue;
        };
        if partner < i {
            continue;
        }
        elements.push((
            i,
            format!(
                "<p>{{move || if flag.get() {{ {} }} else {{ {} }}}}</p>",
                render(i)?,
                render(partner)?
            ),
        ));
    }
    let deferred = by(Shape::Deferred);
    let mut table = String::new();
    if let Some(&first) = deferred.first() {
        let tname = format!("T{name}");
        let _ = writeln!(table, "pub static {tname}: &[Row] = &[");
        for (r, &i) in deferred.iter().enumerate() {
            let _ = writeln!(table, "    Row {{ key: \"k{r}\", label: {} }},", render(i)?);
        }
        table.push_str("];\n\n");
        let view = template.deferred_view("row.label")?;
        let title = template.deferred_string(&format!("crate::tables::{tname}[0].label"))?;
        elements.push((
            first,
            format!(
                "<ul class=\"table\" title={{{title}}}>\n                    {{crate::tables::{tname}.iter().map(|row| view! {{ <li>{{{view}}}</li> }}).collect_view()}}\n                </ul>"
            ),
        ));
    }
    elements.sort_by_key(|e| e.0);

    let mut code = format!(
        "//! Component C{name} (generated by workload-gen; do not edit).\n\nuse leptos::prelude::*;\n\nuse crate::widgets::{{SignalHint, StringBadge, TextLabel}};\n{}\n\n",
        template.prelude
    );
    code.push_str(&helpers);
    let _ = write!(
        code,
        "#[component]\npub fn C{name}() -> impl IntoView {{\n    let (count, set_count) = signal(1_i64);\n    let (name, set_name) = signal(String::from(\"Ada\"));\n    let (stamp, set_stamp) = signal(String::from(\"2026-09-20T12:00:00\"));\n    let (flag, set_flag) = signal(false);\n    let (error, set_error) = signal(String::new());\n    let n = count.get_untracked();\n    let who = crate::widgets::pick_name(n);\n    let when = crate::widgets::pick_time(n);\n    let mut blocks: Vec<AnyView> = Vec::new();\n"
    );
    for block in elements.chunks(BLOCK) {
        code.push_str("    blocks.push(\n        view! {\n            <div class=\"block\">\n");
        for (_, element) in block {
            let _ = writeln!(code, "                {element}");
        }
        code.push_str("            </div>\n        }\n        .into_any(),\n    );\n");
    }
    let _ = write!(
        code,
        "    view! {{\n        <section class=\"component\" aria-label=\"Component {c}\">\n            {{blocks}}\n            <p class=\"status\" role=\"status\">{{move || error.get()}}</p>\n        </section>\n    }}\n}}\n"
    );
    Ok((code, table))
}

fn tables_rs(template: &Template, tables: &str) -> String {
    format!(
        "//! Deferred labels: one `static` table per component (generated by\n//! workload-gen; do not edit).\n\nuse leptos::prelude::*;\n{}\n\n/// One row of a deferred-label table.\npub struct Row {{\n    /// Row key.\n    pub key: &'static str,\n    /// The label, in the template's deferred type.\n    pub label: {},\n}}\n\n{tables}",
        template.prelude,
        template.deferred_type()
    )
}

fn cargo_toml(wl: &Workload, template: &Template, krate: &str) -> String {
    use versions::{AXUM, TOKIO, WASM_BINDGEN};
    let (leptos, leptos_router, leptos_axum) = versions::leptos(template.leptos);
    let mut extra = String::new();
    for line in &template.dependencies {
        extra.push_str(line);
        extra.push('\n');
    }
    // What the template's own crates need forwarded to them. Each entry
    // carries its **own** separator, so that two of them do not produce
    // `,,` — which a template with one entry never showed.
    let feature = |name: &str, indent: &str| -> String {
        template
            .features
            .get(name)
            .map(|entries| {
                let mut out = String::new();
                for e in entries {
                    let _ = write!(out, ",{indent}{e:?}");
                }
                out
            })
            .unwrap_or_default()
    };
    // Leptos 0.9 hydrates a `#[lazy_route]` only with its `lazy` feature
    // (tachys panics at hydration without it); 0.8 has no such feature.
    let lazy = match template.leptos {
        crate::template::LeptosLine::V0_9 => r#", "leptos/lazy""#,
        crate::template::LeptosLine::V0_8 => "",
    };
    let hydrate = format!("{lazy}{}", feature("hydrate", " "));
    let ssr = feature("ssr", "\n    ");
    format!(
        r#"# Reference-workload app, generated by workload-gen; do not edit.
# Knobs: {summary}
# Template: {tname} — {tdesc}

[package]
name = "{krate}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
leptos = "{leptos}"
leptos_router = "{leptos_router}"
leptos_axum = {{ version = "{leptos_axum}", optional = true }}
axum = {{ version = "{AXUM}", optional = true }}
tokio = {{ version = "{TOKIO}", features = ["rt-multi-thread", "macros"], optional = true }}
wasm-bindgen = "{WASM_BINDGEN}"
{extra}
[features]
hydrate = ["leptos/hydrate"{hydrate}]
ssr = [
    "dep:axum",
    "dep:tokio",
    "dep:leptos_axum",
    "leptos/ssr",
    "leptos_router/ssr"{ssr}
]

# Size-optimised client wasm (plans/06-size-and-perf.md §3); measure after
# `wasm-opt -Oz`.
[profile.wasm-release]
inherits = "release"
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

[package.metadata.leptos]
output-name = "{krate}"
site-root = "target/site"
site-pkg-dir = "pkg"
style-file = "style/main.css"
site-addr = "127.0.0.1:3000"
reload-port = 3001
bin-features = ["ssr"]
bin-default-features = false
lib-features = ["hydrate"]
lib-default-features = false
lib-profile-release = "wasm-release"

# Standalone: not a member of the rust-mf2 workspace.
[workspace]
"#,
        summary = wl.knobs.summary(),
        tname = template.name,
        tdesc = template.description,
    )
}

fn lib_rs(template: &Template) -> String {
    format!(
        r#"//! Reference-workload app (generated by workload-gen; do not edit).
//! Template: `{}`.

#![recursion_limit = "512"]
#![allow(unused, clippy::all, clippy::pedantic)]

pub mod app;
pub mod components;
pub mod support;
pub mod tables;
pub mod widgets;

/// Client entry point; `hydrate_lazy` supports `#[lazy_route]` chunks under
/// `cargo leptos build --split`.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {{
    {}
    leptos::mount::hydrate_lazy(app::App);
}}
"#,
        template.name, template.boot
    )
}

fn app_rs(plan: &SitePlan, names: &[String], krate: &str, template: &Template) -> String {
    let r = plan.routes;
    let mut out = String::from(
        r#"//! Shell, router and routes (generated by workload-gen; do not edit).

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{Lazy, LazyRoute, StaticSegment, lazy_route};

use crate::components::*;

PROVIDER_USE/// The HTML shell rendered by the server.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en" dir="ltr">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>"Reference workload"</title>
                <link rel="stylesheet" href="/pkg/CRATE.css"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

/// The application: one eager home route and the lazy routes.
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <header>
                <nav aria-label="Routes">
                    <ul class="nav">
                        <li><a href="/">"Home"</a></li>
"#,
    );
    out = out.replace("CRATE", krate);
    // A template whose library keeps its state in a context wraps the
    // router in the component that provides it.
    out = match &template.provider {
        Some(provider) => out
            .replace(
                "PROVIDER_USE",
                &format!("use crate::support::{provider};\n\n"),
            )
            .replace(
                "    view! {\n        <Router>\n",
                &format!("    view! {{\n        <{provider}>\n        <Router>\n"),
            ),
        None => out.replace("PROVIDER_USE", ""),
    };
    for route in 1..=r {
        let _ = writeln!(
            out,
            "                        <li><a href=\"/r{route}\">\"Route {route}\"</a></li>"
        );
    }
    out.push_str(
        r#"                    </ul>
                </nav>
            </header>
            <main>
                <Routes fallback=|| "Not found.">
                    <Route path=StaticSegment("") view=Home/>
"#,
    );
    for route in 1..=r {
        let _ = writeln!(
            out,
            "                    <Route path=StaticSegment(\"r{route}\") view={{Lazy::<Route{route}>::new()}}/>"
        );
    }
    out.push_str("                </Routes>\n            </main>\n        </Router>\n");
    if let Some(provider) = &template.provider {
        let _ = writeln!(out, "        </{provider}>");
    }
    out.push_str("    }\n}\n");

    let parts = |route: usize, indent: &str| -> String {
        let mut s = format!("{indent}let parts: Vec<AnyView> = vec![\n");
        for (c, name) in names.iter().enumerate() {
            if plan.route_of(c) == route {
                let _ = writeln!(s, "{indent}    view! {{ <C{name}/> }}.into_any(),");
            }
        }
        s.push_str(indent);
        s.push_str("];\n");
        s
    };
    let _ = write!(
        out,
        "\n/// The eager home route.\n#[component]\nfn Home() -> impl IntoView {{\n{}    view! {{ <div class=\"route\">{{parts}}</div> }}\n}}\n",
        parts(0, "    ")
    );
    for route in 1..=r {
        let body = parts(route, "        ");
        let _ = write!(
            out,
            "\n/// Lazy route {route}: its components load in their own chunk under `--split`.\n#[derive(Debug)]\npub struct Route{route};\n\n#[lazy_route]\nimpl LazyRoute for Route{route} {{\n    fn data() -> Self {{\n        Self\n    }}\n\n    fn view(this: Self) -> AnyView {{\n        let _ = this;\n{body}        view! {{ <div class=\"route\">{{parts}}</div> }}.into_any()\n    }}\n}}\n"
        );
    }
    out
}

const MAIN_RS: &str = r#"//! Axum server (generated by workload-gen; do not edit).

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use workload_app::app::{App, shell};

    let conf = get_configuration(None).expect("leptos configuration");
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);
    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("serve");
}

#[cfg(not(feature = "ssr"))]
fn main() {}
"#;

const WIDGETS_RS: &str = r#"//! Shared widgets and plain argument values (generated by workload-gen).

use leptos::prelude::*;

const NAMES: [&str; 4] = ["Ada", "Lin", "Sam", "Kai"];
const TIMES: [&str; 2] = ["2026-09-20T12:00:00", "2026-01-02T08:30:00"];

/// A plain string argument value, chosen at run time.
pub fn pick_name(n: i64) -> &'static str {
    NAMES[(n.unsigned_abs() % 4) as usize]
}

/// A plain date/time argument value, chosen at run time.
pub fn pick_time(n: i64) -> &'static str {
    TIMES[(n.unsigned_abs() % 2) as usize]
}

/// Reactive text through `TextProp`.
#[component]
pub fn TextLabel(#[prop(into)] text: TextProp) -> impl IntoView {
    view! { <span class="label">{move || text.get()}</span> }
}

/// Reactive text through `Signal<String>`.
#[component]
pub fn SignalHint(#[prop(into)] text: Signal<String>) -> impl IntoView {
    view! { <small class="hint">{text}</small> }
}

/// Plain `String` text.
#[component]
pub fn StringBadge(#[prop(into)] text: String) -> impl IntoView {
    view! { <span class="badge">{text}</span> }
}
"#;

const CSS: &str = r"/* Reference-workload app (generated by workload-gen). Flexbox only. */
body {
    margin: 0;
    font-family: system-ui, sans-serif;
    color: #1a1a1a;
    background: #ffffff;
}
header,
main {
    padding: 1rem;
}
.nav {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    margin: 0;
    padding: 0;
    list-style: none;
}
.route,
.component {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}
.block {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.5rem;
}
button,
input {
    min-height: 24px;
    min-width: 24px;
}
a:focus-visible,
button:focus-visible,
input:focus-visible {
    outline: 2px solid #1a4fd6;
    outline-offset: 2px;
}
";

/// `sites.json`: one object per call site, for probes that report per shape.
pub fn sites_json(wl: &Workload, plan: &SitePlan) -> String {
    let mut out = String::from("[\n");
    for (i, site) in plan.sites.iter().enumerate() {
        let message = &wl.messages[site.message];
        let ctx = site_ctx(wl, plan, i);
        let args: Vec<String> = ctx.args.iter().map(|a| format!("\"{}\"", a.name)).collect();
        let _ = write!(
            out,
            "  {{\"site\": {i}, \"component\": {}, \"route\": {}, \"shape\": \"{}\", \"mode\": \"{}\", \"index\": {}, \"id\": ",
            site.component,
            plan.route_of(site.component),
            site.shape.key(),
            site.mode.key(),
            wl.index[site.message],
        );
        crate::json::string(&mut out, &message.id);
        let _ = write!(out, ", \"args\": [{}]}}", args.join(", "));
        out.push_str(if i + 1 == plan.sites.len() {
            "\n"
        } else {
            ",\n"
        });
    }
    out.push_str("]\n");
    out
}
