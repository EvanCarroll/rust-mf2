#!/usr/bin/env python3
"""A9: put one case into a client's source, or put the source back.

    case.py apply CLIENT CASE    (run from the root of the tree)
    case.py restore CLIENT
    case.py show CLIENT CASE     (print the snippet only)

A case is a working-tree edit, never committed: one statement after a fixed
line of the client's entry point, reached by every build of that client.

    base              nothing (the file as it is)
    display-T         black_box(format!("{}", x))
    debug-T           black_box(format!("{:?}", x))
    both-T            black_box(format!("{} {:?}", x, x))
    tostring-T        black_box(x.to_string())        (the inherent, fmt-free)
    control           A5's positive control: format!("{} {:?}", a Tr, a TrArgs)

where T is tr, trargs, trrich or trdyn and x a black-boxed description of that
type. Clients whose `leptos-mf2` has no Leptos mode (fixture, tr) get a rich
description through `mf2::Handler`; the others through a view closure, as an
application writes it.
"""
import pathlib
import subprocess
import sys

MARK = "// A9 case"

# client: (file, anchor line, i18n crate, leptos mode, messages)
CLIENTS = {
    "fixture": (
        "tools/i18n-fixture/src/main.rs",
        "    std::hint::black_box(save.id().raw());",
        "mf2_i18n_fixture",
        False,
        {
            "tr": '"plain"',
            "trargs": '"items", count = 2',
            "trrich": '"help", kbd = mf2::Handler(A9Handler), b = mf2::Handler(A9Handler)',
            "trdyn": ('"greeting"', '[("name", "Ada")]'),
        },
    ),
    "tr": (
        "target/size/wl-1860/app-tr/src/lib.rs",
        "    crate::support::boot();",
        "workload_i18n",
        False,
        {
            "tr": '"profile.id-count-search-search"',
            "trargs": '"admin.labels.disabled", "item" = "Ada"',
            "trrich": '"chat.table.open-members", kbd = mf2::Handler(A9Handler)',
            "trdyn": ('"admin.labels.disabled"', '[("item", "Ada")]'),
        },
    ),
    "tr-view": (
        "target/b5/wl-view-1860/app-tr-view/src/lib.rs",
        "    crate::support::boot();",
        "workload_i18n",
        True,
        {
            "tr": '"profile.id-count-search-search"',
            "trargs": '"admin.labels.disabled", "item" = "Ada"',
            "trrich": '"chat.table.open-members", kbd = |children: AnyView| view! { <kbd>{children}</kbd> }',
            "trdyn": ('"admin.labels.disabled"', '[("item", "Ada")]'),
        },
    ),
    "demo-ssr": (
        "examples/demo-ssr/src/lib.rs",
        "    leptos_mf2::install(demo_i18n::setup());",
        "demo_i18n",
        True,
        None,
    ),
    "demo-csr": (
        "examples/demo-csr/src/main.rs",
        "    leptos_mf2::install(demo_csr_i18n::setup());",
        "demo_csr_i18n",
        True,
        None,
    ),
    # demo-ssr on Leptos 0.8: A7's copy (target/a9/demos-0-8.py a9), for
    # type-checks only (A9_CHECK=1).
    "demo-ssr-08": (
        "target/a7-demo-0-8/a9/examples/demo-ssr/src/lib.rs",
        "    leptos_mf2::install(demo_i18n::setup());",
        "demo_i18n",
        True,
        None,
    ),
    "demo-islands": (
        "examples/demo-islands/src/lib.rs",
        "    leptos_mf2::install(demo_islands_i18n::setup());",
        "demo_islands_i18n",
        True,
        None,
    ),
}

# The three demos share their corpus's shape.
DEMO_MESSAGES = {
    "tr": '"app-title"',
    "trargs": '"people-online", count = 3',
    "trrich": '"hotkey", kbd = |children: AnyView| view! { <kbd>{children}</kbd> }',
    "trdyn": ('"people-online"', '[("count", 3)]'),
}

FORMS = {
    "display": 'format!("{}", x)',
    "debug": 'format!("{:?}", x)',
    "both": 'format!("{} {:?}", x, x)',
    "tostring": "x.to_string()",
}

HANDLER = (
    "struct A9Handler;\n"
    "impl mf2::MarkupHandler for A9Handler {\n"
    "    fn as_any(&self) -> &dyn std::any::Any {\n"
    "        self\n"
    "    }\n"
    "}\n"
)


def description(client, kind):
    _, _, crate, _, messages = CLIENTS[client]
    messages = messages or DEMO_MESSAGES
    m = messages[kind]
    if kind == "trdyn":
        mid, args = m
        return f"mf2::TrDyn::new({crate}::msg_id!({mid}), {args})"
    return f"{crate}::tr!({m})"


# The silent paths (demo clients only): a description reaching `Display` with
# no `format!` in the application.
#   silent-new    paths that 1.x refused at compile time: Leptos API bounded
#                 on `Display` / `ToString`, and the application's own generic
#                 code
#   silent-traps  code that compiles in 1.x too, where `.to_string()` on a
#                 wrapper whose `Display` forwards to the description resolves
#                 to the wrapper's blanket `ToString` before auto-deref reaches
#                 the description's inherent, fmt-free `to_string()`. Each
#                 trap uses another description type, so that the names in a
#                 build show which `Display` each one reached.
def silent(client, case):
    crate = CLIENTS[client][2]
    m = DEMO_MESSAGES
    if case == "silent-new":
        return [
            "use leptos::prelude::*;",
            "use leptos_router::components::{ProtectedRoute, Redirect, Router, Routes};",
            "// 1. leptos_router's <Redirect path=…/>: P: Display, then path.to_string()",
            f"let _ = view! {{ <Redirect path={crate}::tr!({m['tr']}) /> }};",
            "// 2. <ProtectedRoute redirect_path=…/>: Fn() -> P, P: Display",
            "let _ = view! {",
            "    <Router>",
            "        <Routes fallback=|| ()>",
            f"            <ProtectedRoute path=leptos_router::path!(\"/p\") condition=|| Some(false) redirect_path=|| {crate}::tr!({m['tr']}) view=|| () />",
            "        </Routes>",
            "    </Router>",
            "};",
            "// 3. server_fn's ServerFnError::new(impl ToString)",
            f"std::hint::black_box(ServerFnError::new({crate}::tr!({m['tr']})));",
            "// 4. the application's own generic code, bounded on ToString",
            "fn label(x: impl ToString) -> String {",
            "    x.to_string()",
            "}",
            f"std::hint::black_box(label({crate}::tr!({m['tr']})));",
            "// 5. either_of's Either<A: Display, B: Display>: Display",
            f"std::hint::black_box(leptos::either::Either::<mf2::Tr, mf2::TrArgs>::Left({crate}::tr!({m['tr']})).to_string());",
            "// 6. leptos_router's StaticParamsMap::insert(key: impl ToString, …)",
            "let mut params = leptos_router::static_routes::StaticParamsMap::default();",
            f"params.insert({crate}::tr!({m['tr']}), Vec::new());",
            "std::hint::black_box(params);",
        ]
    if case == "silent-traps":
        return [
            "use leptos::prelude::*;",
            "// 1. a signal's read guard (reactive_graph: ReadGuard<T: Display>: Display), a Tr",
            f"let signal = RwSignal::new({crate}::tr!({m['tr']}));",
            "std::hint::black_box(signal.read().to_string());",
            "// 2. a smart pointer (std: Arc<T: Display>: Display), a TrArgs",
            f"let shared = std::sync::Arc::new({crate}::tr!({m['trargs']}));",
            "std::hint::black_box(shared.to_string());",
            "// 3. a RefCell borrow (std: Ref<T: Display>: Display), a TrRich",
            f"let cell = std::cell::RefCell::new({crate}::tr!({m['trrich']}));",
            "std::hint::black_box(cell.borrow().to_string());",
            "// 4. a reference to a reference (std: &T: Display), a TrDyn — what",
            "//    `.iter().find(|d| d.to_string() == …)` hands a closure",
            f"let named = mf2::TrDyn::new({crate}::msg_id!({m['trdyn'][0]}), {m['trdyn'][1]});",
            "let twice = &&named;",
            "std::hint::black_box(twice.to_string());",
        ]
    # One trap per case, so that each one's cost and the check's view of it
    # stand alone (LLVM inlines some of the Display chain into the caller).
    traps = {
        "trap-guard": [
            f"let signal = RwSignal::new({crate}::tr!({m['tr']}));",
            "std::hint::black_box(signal.read().to_string());",
        ],
        "trap-arc": [
            f"let shared = std::sync::Arc::new({crate}::tr!({m['tr']}));",
            "std::hint::black_box(shared.to_string());",
        ],
        "trap-refcell": [
            f"let cell = std::cell::RefCell::new({crate}::tr!({m['tr']}));",
            "std::hint::black_box(cell.borrow().to_string());",
        ],
        "trap-refref": [
            f"let one = std::hint::black_box({crate}::tr!({m['tr']}));",
            "let twice = &&one;",
            "std::hint::black_box(twice.to_string());",
        ],
    }
    if case.removeprefix("silent-") in traps:
        return ["use leptos::prelude::*;"] + traps[case.removeprefix("silent-")]
    if case == "silent-debug":
        # `Debug` has its own: a panic message formats its payload with it.
        return [
            "// 1. Result::unwrap on a Result whose error is a description (1.x: TrArgs had no Debug)",
            f"let checked: Result<(), mf2::TrArgs> = std::hint::black_box(Err({crate}::tr!({m['trargs']})));",
            "std::hint::black_box(checked.unwrap());",
            "// 2. assert_eq! on descriptions: its failure message is their Debug",
            f"assert_eq!(std::hint::black_box({crate}::tr!({m['tr']})), {crate}::tr!({m['tr']}));",
            "// 3. the application's own type, deriving Debug, in an unwrap",
            "#[derive(Debug)]",
            "struct Invalid {",
            "    #[allow(dead_code)]",
            "    reason: mf2::TrRich,",
            "}",
            "use leptos::prelude::AnyView;",
            f"let form: Result<(), Invalid> = std::hint::black_box(Err(Invalid {{ reason: {crate}::tr!({m['trrich']}) }}));",
            "std::hint::black_box(form.unwrap());",
        ]
    raise SystemExit(f"unknown case {case}")


def snippet(client, case):
    _, _, _, leptos, _ = CLIENTS[client]
    lines = [f"{MARK} {case} (a working-tree edit, never committed)", "{"]
    body = []
    if case.startswith("silent-"):
        lines.extend("    " + b for b in silent(client, case))
        lines.append("}")
        return lines
    if leptos:
        body.append("use leptos::prelude::*;")
    if case == "control":
        body.append(f"let save = std::hint::black_box({description(client, 'tr')});")
        body.append(f"let items = std::hint::black_box({description(client, 'trargs')});")
        body.append('std::hint::black_box(format!("{save} {items:?}"));')
    else:
        form, kind = case.split("-", 1)
        if kind == "trrich" and not leptos:
            body.extend(HANDLER.splitlines())
        body.append(f"let x = std::hint::black_box({description(client, kind)});")
        body.append(f"std::hint::black_box({FORMS[form]});")
    lines.extend("    " + b for b in body)
    lines.append("}")
    return lines


def pristine_text(root, client):
    path, *_ = CLIENTS[client]
    keep = root / "target/a9/pristine" / client / pathlib.Path(path).name
    tracked = subprocess.run(
        ["git", "ls-files", "--error-unmatch", path], cwd=root, capture_output=True
    ).returncode == 0
    if tracked:
        return subprocess.run(
            ["git", "show", f"HEAD:{path}"], cwd=root, capture_output=True, check=True, text=True
        ).stdout
    if not keep.exists():
        text = (root / path).read_text()
        if MARK in text:
            sys.exit(f"{path} carries an A9 case and no pristine copy exists")
        keep.parent.mkdir(parents=True, exist_ok=True)
        keep.write_text(text)
    return keep.read_text()


def main():
    root = pathlib.Path(
        subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
    )
    action, client = sys.argv[1], sys.argv[2]
    path, anchor, *_ = CLIENTS[client]
    target = root / path
    text = pristine_text(root, client)
    if action == "restore":
        if target.read_text() != text:
            target.write_text(text)
        return
    case = sys.argv[3]
    if action == "show":
        print("\n".join(snippet(client, case)))
        return
    if case == "base":
        if target.read_text() != text:
            target.write_text(text)
        return
    lines = text.split("\n")
    hits = [i for i, l in enumerate(lines) if l == anchor]
    if len(hits) != 1:
        sys.exit(f"{path}: the anchor occurs {len(hits)} times")
    indent = anchor[: len(anchor) - len(anchor.lstrip())]
    new = [indent + l for l in snippet(client, case)]
    lines[hits[0] + 1 : hits[0] + 1] = new
    target.write_text("\n".join(lines))


if __name__ == "__main__":
    main()
