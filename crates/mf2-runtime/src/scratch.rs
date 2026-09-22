//! Per-call working lists on the heap, grown only fallibly: every growth
//! goes through [`try_push`] — `try_reserve`, then `push` behind the very
//! test `push` makes (`len < capacity`) — so `push`'s infallible-growth
//! branch, and the capacity-overflow / allocation-failure panic behind it,
//! is provably dead and not linked (B12). That needs `Vec::push` inlined
//! next to the test, which at `opt-level = "z"` LLVM does only for a function
//! with a single caller: so each `Vec<T>::push` has exactly one call site,
//! inside [`Scratch::push`], which is kept out of line (one copy per `T`).
//! A failed reservation is reported to the caller, never an abort.
//!
//! A list that stays empty allocates nothing, so a message without
//! declarations, options or selectors formats without allocating.

use alloc::vec::Vec;

/// Appends `t` to `v` with fallible growth; `false` (and `t` dropped) if the
/// memory could not be reserved. Always inlined: the test must sit next to
/// `push` for the growth branch to be dead (the module docs).
#[allow(clippy::inline_always)]
#[inline(always)]
pub(crate) fn try_push<T>(v: &mut Vec<T>, t: T) -> bool {
    if v.len() == v.capacity() && v.try_reserve(1).is_err() {
        return false;
    }
    if v.len() < v.capacity() {
        v.push(t);
        true
    } else {
        false
    }
}

/// A list of `T` (see the module documentation).
pub(crate) struct Scratch<T>(Vec<T>);

impl<T> Scratch<T> {
    pub(crate) const fn new() -> Self {
        Scratch(Vec::new())
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// Appends `t`; `false` if the memory for it could not be reserved.
    #[inline(never)]
    pub(crate) fn push(&mut self, t: T) -> bool {
        try_push(&mut self.0, t)
    }

    pub(crate) fn get(&self, i: usize) -> Option<&T> {
        self.0.get(i)
    }

    pub(crate) fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        self.0.get_mut(i)
    }

    pub(crate) fn iter(&self) -> core::slice::Iter<'_, T> {
        self.0.iter()
    }
}
