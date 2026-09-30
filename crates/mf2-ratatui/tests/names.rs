//! The shim's names are `mf2::ratatui`'s own items.

use core::any::TypeId;

/// Whether `A` and `B` are one type.
fn same<A: ?Sized + 'static, B: ?Sized + 'static>() -> bool {
    TypeId::of::<A>() == TypeId::of::<B>()
}

#[test]
fn each_name_is_the_item_mf2_defines() {
    assert!(same::<mf2_ratatui::Theme, mf2::ratatui::Theme>());
    assert!(same::<mf2_ratatui::Markup, mf2::ratatui::Markup>());
}
