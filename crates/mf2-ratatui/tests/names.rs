//! 1.x's names and paths, as the shim keeps them: each is `mf2`'s own item.

use core::any::TypeId;

use mf2::Tr;
use mf2::native::NativeI18n;
use mf2_ratatui::{MarkupStyles, line, text};

/// Whether `A` and `B` are one type.
fn same<A: ?Sized + 'static, B: ?Sized + 'static>() -> bool {
    TypeId::of::<A>() == TypeId::of::<B>()
}

/// The type of a function that formats a message with 1.x's arguments,
/// here a `Tr`. Every function has a type of its own, so two paths give one
/// type only when they name one function.
fn function<F, R>(_: &F) -> TypeId
where
    F: Fn(&NativeI18n, &Tr, &MarkupStyles) -> R + 'static,
{
    TypeId::of::<F>()
}

#[test]
fn each_1x_name_is_the_item_mf2_defines() {
    assert!(same::<MarkupStyles, mf2::ratatui::MarkupStyles>());
    assert_eq!(function(&line), function(&mf2::ratatui::line));
    assert_eq!(function(&text), function(&mf2::ratatui::text));
}

#[test]
fn two_functions_are_two_types() {
    // The check above means something only if two functions are two types:
    // with the shim's `line` and `text` swapped, it fails.
    assert_ne!(function(&line), function(&text));
}
