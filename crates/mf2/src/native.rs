//! A native application's messages — a command-line tool or a terminal UI,
//! with no Leptos: one generated corpus's catalogs for the whole process,
//! the reader's language chosen from the system's, and the descriptions
//! `tr!` builds shown in it.
//!
//! See the user guide's [native applications page](https://evancarroll.github.io/rust-mf2/native-apps.html).
//!
//! The build script runs `mf2_build` with `Emit::Native` (the catalogs
//! embedded in the executable) or `Emit::NativeFiles` (the catalogs shipped
//! beside it), which generates one [`Corpus`](crate::Corpus) value, `CORPUS`. The
//! application installs it once, at start-up, and a description then shows
//! its text wherever text is wanted: `Display` (`println!("{}", …)`,
//! `format!`), `to_string()`, and `to_cow()`, which borrows a simple
//! message's text from the executable rather than copying it.
//!
//! ```ignore
//! mf2::native::install(&my_i18n::CORPUS); // the system's language, else the source
//! if let Some(lang) = args.lang.as_deref() {
//!     mf2::native::set_locale(lang)?; // an unsupported --lang is an error
//! }
//! println!("{}", my_i18n::tr!("welcome"));
//! ```
//!
//! * **One language for the process**, which [`set_locale`] changes for
//!   every thread's next format; [`with_locale`] gives one thread another
//!   for a scope (a test, a request), and needs no [`install`].
//! * **The settings** — [`set_bidi`], [`set_time_zone`] — apply to every
//!   thread's next format. Bidi isolation is off until set; the time zone
//!   is the system's, by its IANA name, else one that follows the system's
//!   daylight-saving rules, else UTC.
//! * **Before `install`**, in a build whose only mode is `native`, the
//!   ambient forms panic, naming `install()`; in a build with a Leptos mode
//!   beside it, the web's rule holds: the request's or the page's catalog
//!   first, and never a panic.
//! * **[`Catalogs`]** is the explicit form, with no globals:
//!   `catalogs.format("fr", &message)`. [`NativeI18n`], 1.x's handle, is
//!   kept beside it for 1.x applications.
//!
//! Native only: the module is `std`, and reads the system's preferred
//! languages and time zone and files beside the executable. Beside
//! `hydrate` or `csr`, which build the browser's client, the `native`
//! feature is a compile error when compiling for the browser (`wasm32`). On
//! the host the two compile together, so a workspace that holds a browser
//! client and a native application checks as one (`cargo check
//! --workspace`, and rust-analyzer's check).

mod catalogs;
mod handle;
mod locale;
pub(crate) mod store;
mod zone;

pub use crate::error::NativeError as Error;
pub use catalogs::Catalogs;
pub use locale::LocaleSource;
pub use store::{
    bidi, install, install_from_directory, locale, locale_source, set_bidi, set_locale,
    set_time_zone, time_zone, with_locale,
};

/// 1.x's app-owned handle, kept beside the store and [`Catalogs`] for 1.x
/// applications (the `mf2-native` shim) and `mf2::ratatui`'s 1.x functions.
pub use handle::NativeI18n;
