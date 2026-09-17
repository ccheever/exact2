use super::{Lease, Storage, PAGE, WORDS};
use crate::{Component, Material, Transform};
use std::marker::PhantomData;

/// Components whose entire object representation can be uploaded as bytes.
///
/// # Safety
/// A value must have no padding or uninitialized bytes, no interior mutability,
/// and no pointers/references. All-zero bits must be a valid value. Copying a
/// value must preserve initialization of every byte. Prefer a repr(C) layout
/// composed solely of fixed-width numeric fields and assert its size/offsets.
pub unsafe trait Plain: Component + Copy {}
// SAFETY: scalar-math Vec3/Quat and repr(C) Transform contain ten contiguous f32s.
unsafe impl Plain for Transform {}
// SAFETY: repr(C) Material contains ten contiguous f32s, including an explicit pad.
unsafe impl Plain for Material {}

/// Shared lease over a storage's allocated pages. Page views borrow this lease.
///
/// ```compile_fail
/// use exact_game::{World, Transform};
/// let mut world = World::new(60, 0);
/// world.spawn((Transform::default(),));
/// let pages = world.pages::<Transform>();
/// let view = pages.iter().next().unwrap();
/// drop(pages);
/// let bytes = view.bytes();
/// ```
pub struct Pages<'w, C> {
    storage: Option<&'w Storage<C>>,
    _lease: Option<Lease<'w>>,
}
impl<'w, C: Component> Pages<'w, C> {
    pub(crate) fn new(storage: Option<&'w Storage<C>>) -> Self {
        Self {
            storage,
            _lease: storage.map(|s| s.lease(false, 0)),
        }
    }
    /// Allocated pages in ascending entity-index order, skipping freed pages.
    pub fn iter(&self) -> impl Iterator<Item = Page<'_, C>> {
        self.storage.into_iter().flat_map(|s| {
            s.pages.iter().enumerate().filter_map(|(i, slots)| {
                slots.as_ref().map(|slots| Page {
                    first: (i * PAGE) as u32,
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
    /// Sixteen presence words; bit zero corresponds to `first`.
    pub mask: &'a [u64],
    slots: *const C,
    _life: PhantomData<&'a C>,
}
impl<C> Page<'_, C> {
    /// Pointer to PAGE consecutive slots, valid while this view's lease is alive.
    /// Only present slots may be read as C; absent slots are MaybeUninit backing.
    /// Use bytes() for a Plain byte upload. The pointer cannot replace the backing
    /// used by the safe byte view.
    pub fn as_ptr(&self) -> *const C {
        self.slots
    }
}
impl<C: Plain> Page<'_, C> {
    /// All PAGE slots as initialized bytes, including zeroed absent slots.
    /// This is a machine-layout upload view, not the portable save representation.
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: Plain forbids padding/uninitialized bytes and interior mutability;
        // pages start zeroed and removal rezeros slots. The borrowed shared lease
        // excludes writers and keeps the whole allocation alive, including for ZSTs.
        unsafe { std::slice::from_raw_parts(self.slots.cast(), PAGE * std::mem::size_of::<C>()) }
    }
}

impl<C> Page<'_, C> {
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
impl Page<'_, Transform> {
    /// Present runs as ten floats per transform, ready for a renderer's byte copy.
    pub fn float_runs(&self) -> impl Iterator<Item = (u32, &[f32])> {
        self.runs().map(|(first, values)| {
            // SAFETY: Transform's asserted repr(C) layout is ten adjacent f32s;
            // this run contains initialized values and shares the page lease.
            (first, unsafe {
                std::slice::from_raw_parts(values.as_ptr().cast(), values.len() * 10)
            })
        })
    }
}
