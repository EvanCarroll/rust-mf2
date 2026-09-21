//! Support module of the `tr` template (P0.1): client boot only. The stub
//! catalog is filled at run time from `<html data-catalog>`, so every call
//! site's id reaches an opaque lookup, as it would with a fetched catalog.

/// Client boot: installs the catalog before hydration (plans/04 §6).
pub fn boot() {
    #[cfg(feature = "hydrate")]
    mf2_probe::boot_from_dom();
}
