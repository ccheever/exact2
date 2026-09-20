use super::{Lease, Storage, PAGE, WORDS};
use crate::Component;
use std::marker::PhantomData;

/// Shared lease over a storage's allocated pages. Page views borrow this lease.
///
pub struct Pages<'w, C> {
    storage: Option<&'w Storage<C>>,
    _lease: Option<Lease<'w>>,
}
impl<'w, C: Component> Pages<'w, C> {
    pub(crate) fn new(storage: Option<&'w Storage<C>>) -> Self {
        Self {
            storage,
            _lease: storage.map(|s| s.lease(false)),
        }
    }
    /// Allocated pages in ascending entity-index order, skipping freed pages.
    pub fn iter(&self) -> impl Iterator<Item = Page<'_, C>> {
        self.storage.into_iter().flat_map(|s| {
            s.pages.iter().enumerate().filter_map(|(i, slots)| {
                slots.as_ref().map(|slots| Page {
                    first: (i * PAGE) as u32,
                    generation: s.generations[i].get(),
                    mask: &s.mask[i * WORDS..(i + 1) * WORDS],
                    slots: slots.get().cast::<C>(),
                    _life: PhantomData,
                })
            })
        })
    }
}

/// One allocated page, valid while its Pages lease remains borrowed.
pub struct Page<'a, C> {
    /// Entity index of the first of PAGE slots.
    pub first: u32,
    /// Conservative write generation; changes when a mutable row is handed out,
    /// inserted or removed. Compare only within one world presentation generation.
    pub generation: u64,
    /// PAGE / 64 presence words; bit zero corresponds to `first`.
    mask: &'a [u64],
    slots: *const C,
    _life: PhantomData<&'a C>,
}
impl<C> Page<'_, C> {
    pub fn mask(&self) -> &[u64] {
        self.mask
    }
    /// Present contiguous runs, with absolute first-slot indices. A run never
    /// crosses an absent slot, so its values need no MaybeUninit or unsafe caller.
    pub fn runs(&self) -> impl Iterator<Item = (u32, &[C])> {
        let mut at = 0;
        std::iter::from_fn(move || {
            let present = |i: usize| self.mask[i / 64] & (1 << (i % 64)) != 0;
            while at < PAGE && !present(at) {
                at += 1;
            }
            if at == PAGE {
                return None;
            }
            let start = at;
            while at < PAGE && present(at) {
                at += 1;
            }
            // SAFETY: every bit in this run is present and the page holds a shared
            // lease. Unlike bytes(), this works for components with owned fields.
            Some((self.first + start as u32, unsafe {
                std::slice::from_raw_parts(self.slots.add(start), at - start)
            }))
        })
    }
}
