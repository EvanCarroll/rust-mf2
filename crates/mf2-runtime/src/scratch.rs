//! Per-call working lists: inline up to `N` items (no allocation for the
//! messages real corpora have), on the heap beyond. Growth only ever goes
//! through `try_reserve` followed by `push` — std then knows the capacity
//! is there, so no infallible-growth or abort path is linked (B12); a failed
//! reservation is reported to the caller, never an abort.

use alloc::vec::Vec;

/// A list of `T`: inline up to `N`, then on the heap.
pub(crate) enum Scratch<T, const N: usize> {
    Inline { items: [Option<T>; N], len: usize },
    Heap(Vec<T>),
}

impl<T, const N: usize> Scratch<T, N> {
    pub(crate) const fn new() -> Self {
        Scratch::Inline {
            items: [const { None }; N],
            len: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Scratch::Inline { len, .. } => *len,
            Scratch::Heap(v) => v.len(),
        }
    }

    /// Appends `t`; `false` if the memory for it could not be reserved.
    pub(crate) fn push(&mut self, t: T) -> bool {
        match self {
            Scratch::Inline { items, len } => {
                if let Some(slot) = items.get_mut(*len) {
                    *slot = Some(t);
                    *len += 1;
                    return true;
                }
                let mut v = Vec::new();
                if v.try_reserve(N.saturating_mul(2).max(8)).is_err() {
                    return false;
                }
                for slot in items.iter_mut() {
                    if let Some(x) = slot.take()
                        && v.try_reserve(1).is_ok()
                    {
                        v.push(x);
                    }
                }
                if v.try_reserve(1).is_err() {
                    return false;
                }
                v.push(t);
                *self = Scratch::Heap(v);
                true
            }
            Scratch::Heap(v) => {
                if v.try_reserve(1).is_err() {
                    return false;
                }
                v.push(t);
                true
            }
        }
    }

    pub(crate) fn get(&self, i: usize) -> Option<&T> {
        match self {
            Scratch::Inline { items, len } => {
                if i < *len {
                    items.get(i)?.as_ref()
                } else {
                    None
                }
            }
            Scratch::Heap(v) => v.get(i),
        }
    }

    pub(crate) fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        match self {
            Scratch::Inline { items, len } => {
                if i < *len {
                    items.get_mut(i)?.as_mut()
                } else {
                    None
                }
            }
            Scratch::Heap(v) => v.get_mut(i),
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        (0..self.len()).filter_map(move |i| self.get(i))
    }
}
