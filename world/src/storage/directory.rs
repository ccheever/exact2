//! One metadata allocation: contiguous presence words followed by chunk pointers.
//! Values have separate, uninitialized allocations and never move with metadata.
use super::{raw::Bytes, PAGE};
use std::{alloc::Layout, cell::Cell};

pub(super) struct Chunk {
    pub(super) ptr: *mut u8,
    pub(super) generation: Cell<u64>,
}
pub(super) struct Directory {
    bytes: Bytes,
    pub(super) layout: Layout,
    len: usize,
    chunks: *mut Chunk,
}
impl Directory {
    pub(super) fn new(layout: Layout) -> Self {
        Self {
            bytes: Bytes::new(Self::shape(0).0),
            layout,
            len: 0,
            chunks: std::ptr::NonNull::dangling().as_ptr(),
        }
    }
    fn shape(len: usize) -> (Layout, usize) {
        Layout::array::<u64>(len)
            .unwrap()
            .extend(Layout::array::<Chunk>(len).unwrap())
            .unwrap()
    }
    #[inline]
    pub(super) fn mask(&self) -> &[u64] {
        // SAFETY: the aligned prefix contains len initialized words; growth is exclusive.
        unsafe { std::slice::from_raw_parts(self.bytes.get().cast(), self.len) }
    }
    pub(super) fn mask_mut(&mut self) -> &mut [u64] {
        // SAFETY: same prefix invariant, now exclusively borrowed.
        unsafe { std::slice::from_raw_parts_mut(self.bytes.get().cast(), self.len) }
    }
    #[inline]
    pub(super) fn chunks(&self) -> &[Chunk] {
        // SAFETY: Layout::extend aligns the initialized suffix, disjoint from the mask.
        unsafe { std::slice::from_raw_parts(self.chunks, self.len) }
    }

    fn capacity_for(&self, index: usize) -> usize {
        (self.len * 2)
            .max(index + 1)
            .max(4)
            .min(crate::MAX_ENTITIES.div_ceil(PAGE))
    }
    pub(super) fn growth_bytes(&self, index: usize) -> usize {
        if index < self.len {
            0
        } else {
            Self::shape(self.capacity_for(index)).0.size()
        }
    }
    pub(super) fn ensure(&mut self, index: usize) {
        assert!(index < crate::MAX_ENTITIES.div_ceil(PAGE), "chunk limit");
        if index < self.len {
            return;
        }
        let len = self.capacity_for(index);
        let (layout, offset) = Self::shape(len);
        let bytes = Bytes::new(layout);
        // SAFETY: both regions are in the new allocation, aligned and disjoint.
        // Only metadata is initialized. Copies transfer pointer ownership; Chunk
        // has no Drop, and replacing old Bytes frees only the metadata allocation.
        unsafe {
            let mask = bytes.get().cast::<u64>();
            let chunks = bytes.get().add(offset).cast::<Chunk>();
            std::ptr::copy_nonoverlapping(self.mask().as_ptr(), mask, self.len);
            std::ptr::copy_nonoverlapping(self.chunks, chunks, self.len);
            // Zero is valid for u64, raw pointers and Cell<u64>. No component
            // value allocation is zeroed: only the newly added metadata suffixes.
            std::ptr::write_bytes(mask.add(self.len), 0, len - self.len);
            std::ptr::write_bytes(chunks.add(self.len), 0, len - self.len);
        }
        self.chunks = bytes.get().wrapping_add(offset).cast();
        self.bytes = bytes;
        self.len = len;
    }
    pub(super) fn allocate(&mut self, index: usize) {
        self.ensure(index);
        if self.chunks()[index].ptr.is_null() {
            let bytes = Bytes::new(self.layout);
            // SAFETY: exclusive access to the initialized directory slot. This
            // directory owns the allocation until free/Drop, including aligned ZSTs.
            unsafe {
                (*self.chunks.add(index)).ptr = bytes.get();
            }
            std::mem::forget(bytes);
        }
    }
    pub(super) fn free(&mut self, index: usize) {
        // SAFETY: the caller cleared all presence bits and transferred/dropped all
        // values in this chunk. Directory exclusively owns the matching allocation.
        unsafe {
            let chunk = &mut *self.chunks.add(index);
            if !chunk.ptr.is_null() && self.layout.size() != 0 {
                std::alloc::dealloc(chunk.ptr, self.layout);
            }
            chunk.ptr = std::ptr::null_mut();
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        for i in 0..self.len {
            self.free(i);
        }
    }
}
