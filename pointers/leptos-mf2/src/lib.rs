//! `leptos-mf2` moved into `mf2` in 2.0: see the README.

compile_error!(
    "leptos-mf2 moved into the `mf2` crate in 2.0: depend on \
     `mf2 = { version = \"2\", features = [\"leptos\", \"ssr\"] }` instead \
     (one mode: `ssr`, `hydrate` or `csr`; `leptos-0-8` in place of `leptos` \
     for Leptos 0.8). Upgrade guide: \
     https://evancarroll.github.io/rust-mf2/upgrading.html"
);
