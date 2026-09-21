use icu_locale_core::Locale;
use icu_plurals::{PluralCategory, PluralRules};

#[unsafe(no_mangle)]
pub extern "C" fn cat(ptr: *const u8, len: usize, n: u32) -> u32 {
    let s = unsafe { core::slice::from_raw_parts(ptr, len) };
    let Ok(loc) = Locale::try_from_utf8(s) else { return 99 };
    let Ok(pr) = PluralRules::try_new_cardinal(loc.into()) else { return 98 };
    match pr.category_for(n) {
        PluralCategory::Zero => 0,
        PluralCategory::One => 1,
        PluralCategory::Two => 2,
        PluralCategory::Few => 3,
        PluralCategory::Many => 4,
        PluralCategory::Other => 5,
    }
}
