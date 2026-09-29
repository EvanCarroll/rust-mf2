//! 1.x's names and paths, as the shim keeps them: each is `mf2`'s own item.

use core::any::TypeId;

use mf2_native::{
    BidiStrategy, Corpus, Dir, LocaleSource, Message, NativeError, NativeI18n, TimeZone,
};

/// Whether `A` and `B` are one type.
fn same<A: ?Sized + 'static, B: ?Sized + 'static>() -> bool {
    TypeId::of::<A>() == TypeId::of::<B>()
}

#[test]
fn each_1x_name_is_the_item_mf2_defines() {
    assert!(same::<NativeI18n, mf2::native::NativeI18n>());
    assert!(same::<NativeError, mf2::native::NativeError>());
    assert!(same::<LocaleSource, mf2::native::LocaleSource>());
    assert!(same::<BidiStrategy, mf2::BidiStrategy>());
    assert!(same::<Corpus, mf2::Corpus>());
    assert!(same::<Dir, mf2::Dir>());
    assert!(same::<TimeZone, mf2::TimeZone>());
    assert!(same::<dyn Message, dyn mf2::Message>());
}

#[test]
fn locale_source_keeps_its_1x_variants() {
    let all = [
        LocaleSource::Explicit,
        LocaleSource::System,
        LocaleSource::Source,
    ];
    assert_eq!(all.map(|s| all.iter().filter(|t| **t == s).count()), [1; 3]);
}
