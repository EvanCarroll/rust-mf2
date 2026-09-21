//! icu_plurals with a runtime blob (BlobDataProvider), audit-style harness.
#![allow(clippy::missing_safety_doc)]

use icu_locale_core::Locale;
use icu_plurals::{PluralCategory, PluralRules};
use icu_provider_blob::BlobDataProvider;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn cat(ptr: *const u8, len: usize, bptr: *const u8, blen: usize, n: u32) -> u32 {
    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let b = unsafe { core::slice::from_raw_parts(bptr, blen) };
    let Ok(loc) = Locale::try_from_utf8(s) else { return 99 };
    let Ok(provider) = BlobDataProvider::try_new_from_blob(b.to_vec().into_boxed_slice()) else { return 97 };
    let Ok(pr) = PluralRules::try_new_cardinal_with_buffer_provider(&provider, loc.into()) else { return 98 };
    match pr.category_for(n) {
        PluralCategory::Zero => 0,
        PluralCategory::One => 1,
        PluralCategory::Two => 2,
        PluralCategory::Few => 3,
        PluralCategory::Many => 4,
        PluralCategory::Other => 5,
    }
}
