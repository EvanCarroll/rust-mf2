//! Native application support for MF2: load one generated corpus's
//! catalogs, select a supported locale, and format the call-site
//! descriptions `tr!` builds — in a CLI or a terminal UI, without Leptos.
//!
//! See the [native application guide](https://evancarroll.github.io/rust-mf2/native-apps.html).
//!
//! The i18n crate's build script runs `mf2_build` with `Emit::Native`,
//! which generates one `CORPUS` value; the locale then belongs to
//! [`NativeI18n`], not to a process or thread global.
//!
//! ```ignore
//! let mut i18n = NativeI18n::embedded(&my_i18n::CORPUS)?; // the system's locale
//! if let Some(lang) = args.lang.as_deref() {
//!     i18n.set_locale(lang)?; // an unsupported --lang is an error
//! }
//! println!("{}", i18n.format(&my_i18n::tr!("welcome")));
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod error;
mod locale;
mod native;

pub use error::NativeError;
pub use locale::LocaleSource;
pub use native::NativeI18n;

pub use mf2::{BidiStrategy, Corpus, Dir, Message, TimeZone};
