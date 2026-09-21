//! P0.9 stand-in for `mf2-catalog` with its `manifest` and `writer` features
//! (plans/02-catalog-format.md §3, §5). Throwaway probe code.

#![forbid(unsafe_code)]

pub mod catalog;
pub mod error;
pub mod manifest;
pub mod model;
pub mod wire;

pub use catalog::{Catalog, CatalogMessage};
pub use error::DecodeError;
pub use manifest::{Manifest, ManifestEntry};
pub use model::{Body, Func, Key, Part, Selector, Variant};
