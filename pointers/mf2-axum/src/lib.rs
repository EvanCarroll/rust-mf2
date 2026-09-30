//! `mf2-axum` moved into `mf2` in 2.0: see the README.

compile_error!(
    "mf2-axum moved into the `mf2` crate in 2.0: depend on \
     `mf2 = { version = \"2\", features = [\"axum\"] }` instead. \
     Upgrade guide: https://evancarroll.github.io/rust-mf2/upgrading.html"
);
