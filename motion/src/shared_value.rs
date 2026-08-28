//! The shared-value plane: a stable-address, segmented registry of the
//! single-word values drivers and gestures write and sinks read.
//!
//! @ref LLP 0099#SharedValue
//!
//! Identity is a typed `(slot, generation, epoch)` handle, never a pointer.
//! Each slot's `AtomicU64` packs the lifetime generation in the high word and
//! the `f32` bits in the low word, so validate-and-write is a single CAS and a
//! stale handle is refused rather than silently served.
//!
//! Writes belong to the thread that created the slab — the frame thread. That
//! affinity is checked, not assumed: a write from anywhere else is refused with
//! [`SharedValueVerdict::WrongWriterThread`] instead of racing. Readers that
//! register as samplers get a seqlock read that never blocks the writer.
//!
//! A segment is never freed while the slab is live: the fixed directory is
//! allocated once, segment pointers are release-published, and readers
//! acquire-load them. Slot generations always increase, so a freed slot's old
//! handles stay refusable forever.

use std::sync::atomic::{fence, AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering};

/// Slots in one segment.
///
/// At 64 slots per segment and 1024 directory entries a slab grows to 65,536
/// slots without ever moving the directory or a segment.
pub const SHARED_VALUE_SEGMENT_SIZE: u32 = 64;
/// Directory entries, and so the maximum number of segments.
pub const SHARED_VALUE_MAX_SEGMENTS: u32 = 1024;
/// Slots one slab can ever hold.
pub const SHARED_VALUE_MAX_SLOTS: u32 = SHARED_VALUE_SEGMENT_SIZE * SHARED_VALUE_MAX_SEGMENTS;

/// High bit of a generation word: the slot is retired and refuses writes.
const SHARED_VALUE_TOMBSTONE_BIT: u32 = 1 << 31;
const SHARED_VALUE_GENERATION_MASK: u32 = !SHARED_VALUE_TOMBSTONE_BIT;
const SHARED_VALUE_MAX_LIVE_GENERATION: u32 = SHARED_VALUE_GENERATION_MASK - 1;
/// Write count after which a slot retires itself rather than let its
/// per-lifetime write generation wrap.
pub const SHARED_VALUE_WRITE_GENERATION_ESCALATE_AT: u32 = u32::MAX - 1;

/// A typed handle to one shared value.
///
/// All three fields participate in validation: the slot locates the word, the
/// generation identifies which tenant of that slot the holder meant, and the
/// epoch identifies which slab lifetime issued it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SharedValueHandle {
    /// Physical slot index.
    pub slot: u32,
    /// Lifetime generation of the tenant this handle names.
    pub generation: u32,
    /// Slab epoch that issued the handle.
    pub epoch: u32,
}

/// The outcome of a shared-value operation.
///
/// Every refusal is named. A caller never has to distinguish "failed" from
/// "succeeded with a stale value".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedValueVerdict {
    /// The operation applied to the live tenant named by the handle.
    Live,
    /// The slot index has never been allocated.
    StaleSlot,
    /// The slot holds a different tenant than the handle names.
    StaleGeneration,
    /// The handle was issued by a different slab lifetime.
    StaleEpoch,
    /// The tenant named by the handle has been retired.
    Tombstone,
    /// A write arrived from a thread that does not own the value plane.
    WrongWriterThread,
    /// Every directory entry is populated and every slot is in use.
    DirectoryExhausted,
    /// The slot's generation counter cannot advance again.
    GenerationExhausted,
    /// The slot retired itself rather than wrap its write generation.
    WriteGenerationEscalated,
    /// A sampler read was attempted without a registration.
    SamplerNotRegistered,
    /// A non-finite value was offered to the value plane.
    NonFiniteValue,
}

/// One sampler-visible read: the value and the write generation it came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SharedValueSample {
    /// The value observed by this read.
    pub value: f32,
    /// The slot's write count at the moment of the read, so a reader can tell
    /// a fresh publication from a repeated one.
    pub write_generation: u32,
}

#[inline]
fn pack(generation: u32, value: f32) -> u64 {
    ((generation as u64) << 32) | value.to_bits() as u64
}

#[inline]
fn unpack_generation(tagged_word: u64) -> u32 {
    (tagged_word >> 32) as u32
}

#[inline]
fn unpack_value(tagged_word: u64) -> f32 {
    f32::from_bits(tagged_word as u32)
}

#[inline]
fn is_tombstone(generation: u32) -> bool {
    generation & SHARED_VALUE_TOMBSTONE_BIT != 0
}

#[inline]
fn generation_number(generation: u32) -> u32 {
    generation & SHARED_VALUE_GENERATION_MASK
}

#[inline]
fn next_generation(generation: u32) -> Option<u32> {
    let current = generation_number(generation);
    if current >= SHARED_VALUE_GENERATION_MASK {
        None
    } else {
        Some(current + 1)
    }
}

/// One value word plus the metadata a sampler needs to read it safely.
#[derive(Debug)]
struct SharedValueSlot {
    /// Generation in the high word, `f32` bits in the low word.
    tagged_word: AtomicU64,
    write_generation: AtomicU32,
    /// Odd means a sampler-visible publish is in progress. `u64` headroom makes
    /// wrap unreachable for a process lifetime.
    publication_sequence: AtomicU64,
    /// Once a sampler can observe this physical slot, every later mutation
    /// stays sequence-mediated. Unregistering cannot clear this: a sampler that
    /// read the old registration count may still be between its two sequence
    /// loads.
    sampler_sequence_required: AtomicBool,
    sampler_count: AtomicU32,
}

impl SharedValueSlot {
    fn new() -> Self {
        Self {
            tagged_word: AtomicU64::new(pack(SHARED_VALUE_TOMBSTONE_BIT, 0.0)),
            write_generation: AtomicU32::new(0),
            publication_sequence: AtomicU64::new(0),
            sampler_sequence_required: AtomicBool::new(false),
            sampler_count: AtomicU32::new(0),
        }
    }

    fn begin_publication(&self) -> Option<u64> {
        if !self.sampler_sequence_required.load(Ordering::Acquire) {
            return None;
        }
        let mut observed = self.publication_sequence.load(Ordering::Acquire);
        loop {
            if observed & 1 != 0 {
                std::hint::spin_loop();
                observed = self.publication_sequence.load(Ordering::Acquire);
                continue;
            }
            match self.publication_sequence.compare_exchange_weak(
                observed,
                observed + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Pin field stores after the odd marker on weak targets.
                    fence(Ordering::Release);
                    return Some(observed);
                }
                Err(next) => observed = next,
            }
        }
    }

    fn end_publication(&self, even_sequence: Option<u64>) {
        if let Some(sequence) = even_sequence {
            self.publication_sequence
                .store(sequence + 2, Ordering::Release);
        }
    }

    fn activate(
        &self,
        initial_value: f32,
        generation_exhausted: &AtomicBool,
    ) -> Result<u32, SharedValueVerdict> {
        let publication = self.begin_publication();
        let mut observed = self.tagged_word.load(Ordering::Acquire);
        let result = loop {
            let generation = unpack_generation(observed);
            if !is_tombstone(generation) {
                break Err(SharedValueVerdict::StaleGeneration);
            }
            let Some(next) = next_generation(generation) else {
                generation_exhausted.store(true, Ordering::Release);
                break Err(SharedValueVerdict::GenerationExhausted);
            };
            // Keep one final generation number in reserve so every live tenant
            // can still be fenced into a distinct tombstone.
            if next > SHARED_VALUE_MAX_LIVE_GENERATION {
                generation_exhausted.store(true, Ordering::Release);
                break Err(SharedValueVerdict::GenerationExhausted);
            }
            let desired = pack(next, initial_value);
            match self.tagged_word.compare_exchange_weak(
                observed,
                desired,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // A write generation is scoped by the lifetime generation.
                    // Starting every fresh tenant at one avoids carrying a
                    // near-wrap counter across re-tenancy; the lifetime
                    // generation distinguishes the new series.
                    self.write_generation.store(1, Ordering::Release);
                    break Ok(next);
                }
                Err(next) => observed = next,
            }
        };
        self.end_publication(publication);
        result
    }

    fn try_publish(
        &self,
        expected_generation: u32,
        value: f32,
        generation_exhausted: &AtomicBool,
    ) -> SharedValueVerdict {
        if self.write_generation.load(Ordering::Acquire)
            >= SHARED_VALUE_WRITE_GENERATION_ESCALATE_AT
        {
            return self.retire_for_write_generation(expected_generation, generation_exhausted);
        }

        let publication = self.begin_publication();
        let mut observed = self.tagged_word.load(Ordering::Acquire);
        let verdict = loop {
            let generation = unpack_generation(observed);
            if is_tombstone(generation) {
                break SharedValueVerdict::Tombstone;
            }
            if generation != expected_generation {
                break SharedValueVerdict::StaleGeneration;
            }
            let desired = pack(generation, value);
            match self.tagged_word.compare_exchange_weak(
                observed,
                desired,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.write_generation.fetch_add(1, Ordering::Release);
                    break SharedValueVerdict::Live;
                }
                Err(next) => observed = next,
            }
        };
        self.end_publication(publication);
        verdict
    }

    fn retire_for_write_generation(
        &self,
        expected_generation: u32,
        generation_exhausted: &AtomicBool,
    ) -> SharedValueVerdict {
        let publication = self.begin_publication();
        let mut observed = self.tagged_word.load(Ordering::Acquire);
        let verdict = loop {
            let generation = unpack_generation(observed);
            if is_tombstone(generation) {
                break SharedValueVerdict::Tombstone;
            }
            if generation != expected_generation {
                break SharedValueVerdict::StaleGeneration;
            }
            let Some(next) = next_generation(generation) else {
                generation_exhausted.store(true, Ordering::Release);
                break SharedValueVerdict::GenerationExhausted;
            };
            let desired = pack(next | SHARED_VALUE_TOMBSTONE_BIT, unpack_value(observed));
            match self.tagged_word.compare_exchange_weak(
                observed,
                desired,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break SharedValueVerdict::WriteGenerationEscalated,
                Err(next) => observed = next,
            }
        };
        self.end_publication(publication);
        verdict
    }

    fn tombstone(
        &self,
        expected_generation: u32,
        generation_exhausted: &AtomicBool,
    ) -> SharedValueVerdict {
        let publication = self.begin_publication();
        let mut observed = self.tagged_word.load(Ordering::Acquire);
        let verdict = loop {
            let generation = unpack_generation(observed);
            if is_tombstone(generation) {
                break SharedValueVerdict::Tombstone;
            }
            if generation != expected_generation {
                break SharedValueVerdict::StaleGeneration;
            }
            let Some(next) = next_generation(generation) else {
                generation_exhausted.store(true, Ordering::Release);
                break SharedValueVerdict::GenerationExhausted;
            };
            let desired = pack(next | SHARED_VALUE_TOMBSTONE_BIT, unpack_value(observed));
            match self.tagged_word.compare_exchange_weak(
                observed,
                desired,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break SharedValueVerdict::Live,
                Err(next) => observed = next,
            }
        };
        self.end_publication(publication);
        verdict
    }

    fn sample(&self, expected_generation: u32) -> Result<SharedValueSample, SharedValueVerdict> {
        if self.sampler_count.load(Ordering::Acquire) == 0 {
            return Err(SharedValueVerdict::SamplerNotRegistered);
        }
        loop {
            let first = self.publication_sequence.load(Ordering::Acquire);
            if first & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let tagged_word = self.tagged_word.load(Ordering::Relaxed);
            let write_generation = self.write_generation.load(Ordering::Relaxed);
            // The trailing Acquire load alone only orders operations after
            // itself. This fence also keeps the sampled fields before that
            // validation load, which is the reader half of the seqlock.
            fence(Ordering::Acquire);
            let second = self.publication_sequence.load(Ordering::Acquire);
            if first == second && second & 1 == 0 {
                let generation = unpack_generation(tagged_word);
                if is_tombstone(generation) {
                    return Err(SharedValueVerdict::Tombstone);
                }
                if generation != expected_generation {
                    return Err(SharedValueVerdict::StaleGeneration);
                }
                return Ok(SharedValueSample {
                    value: unpack_value(tagged_word),
                    write_generation,
                });
            }
        }
    }
}

struct SharedValueSegment {
    slots: Box<[SharedValueSlot]>,
}

impl SharedValueSegment {
    fn new() -> Self {
        let slots = (0..SHARED_VALUE_SEGMENT_SIZE)
            .map(|_| SharedValueSlot::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { slots }
    }
}

/// The value plane for one motion root: a grow-only, stable-address slab of
/// shared values.
///
/// The slab is shareable across threads — sinks on other threads may register
/// as samplers and read — but every mutation is checked against the creating
/// thread's identity.
pub struct SharedValueSlab {
    directory: Box<[AtomicPtr<SharedValueSegment>]>,
    epoch: u32,
    high_water: AtomicU32,
    live_count: AtomicU32,
    registered_sampler_count: AtomicU32,
    generation_exhausted: AtomicBool,
    writer_thread: std::thread::ThreadId,
}

impl SharedValueSlab {
    /// Create a slab, pre-allocating enough segments for `initial_capacity`
    /// slots. Returns `None` if the request exceeds [`SHARED_VALUE_MAX_SLOTS`].
    ///
    /// The calling thread becomes the value plane's writer thread.
    pub fn new(initial_capacity: u32) -> Option<Self> {
        if initial_capacity > SHARED_VALUE_MAX_SLOTS {
            return None;
        }
        let directory = (0..SHARED_VALUE_MAX_SEGMENTS)
            .map(|_| AtomicPtr::new(std::ptr::null_mut()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let slab = Self {
            directory,
            epoch: 1,
            high_water: AtomicU32::new(0),
            live_count: AtomicU32::new(0),
            registered_sampler_count: AtomicU32::new(0),
            generation_exhausted: AtomicBool::new(false),
            writer_thread: std::thread::current().id(),
        };
        if initial_capacity > 0 {
            let last_segment = (initial_capacity - 1) / SHARED_VALUE_SEGMENT_SIZE;
            for segment in 0..=last_segment {
                slab.ensure_segment(segment)?;
            }
        }
        Some(slab)
    }

    /// Whether the calling thread owns this slab's value plane.
    pub fn is_writer_thread(&self) -> bool {
        std::thread::current().id() == self.writer_thread
    }

    /// The slab lifetime that stamps every handle it issues.
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Slots allocated so far.
    pub fn live_count(&self) -> u32 {
        self.live_count.load(Ordering::Acquire)
    }

    /// Sampler registrations currently outstanding across all slots.
    pub fn registered_sampler_count(&self) -> u32 {
        self.registered_sampler_count.load(Ordering::Acquire)
    }

    fn ensure_segment(&self, segment_index: u32) -> Option<*mut SharedValueSegment> {
        let entry = self.directory.get(segment_index as usize)?;
        let existing = entry.load(Ordering::Acquire);
        if !existing.is_null() {
            return Some(existing);
        }
        let candidate = Box::into_raw(Box::new(SharedValueSegment::new()));
        match entry.compare_exchange(
            std::ptr::null_mut(),
            candidate,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => Some(candidate),
            Err(winner) => {
                // SAFETY: this candidate was never published.
                drop(unsafe { Box::from_raw(candidate) });
                (!winner.is_null()).then_some(winner)
            }
        }
    }

    fn slot(&self, slot: u32) -> Option<&SharedValueSlot> {
        if slot >= SHARED_VALUE_MAX_SLOTS {
            return None;
        }
        let segment_index = slot / SHARED_VALUE_SEGMENT_SIZE;
        let slot_index = slot % SHARED_VALUE_SEGMENT_SIZE;
        let segment = self
            .directory
            .get(segment_index as usize)?
            .load(Ordering::Acquire);
        if segment.is_null() {
            return None;
        }
        // SAFETY: segments are immutable allocations after publication and are
        // released only when the slab is dropped, after quiescence.
        let segment = unsafe { segment.as_ref() }?;
        segment.slots.get(slot_index as usize)
    }

    /// Allocate a value, seeded with `initial_value`.
    ///
    /// Slots are never re-tenanted: a freed slot stays retired for the life of
    /// the slab, so no handle can ever name a value it did not allocate.
    pub fn allocate(&self, initial_value: f32) -> Result<SharedValueHandle, SharedValueVerdict> {
        if !self.is_writer_thread() {
            return Err(SharedValueVerdict::WrongWriterThread);
        }
        if !initial_value.is_finite() {
            return Err(SharedValueVerdict::NonFiniteValue);
        }
        if self.generation_exhausted.load(Ordering::Acquire) {
            return Err(SharedValueVerdict::GenerationExhausted);
        }

        let slot_index = self.high_water.load(Ordering::Relaxed);
        if slot_index >= SHARED_VALUE_MAX_SLOTS {
            return Err(SharedValueVerdict::DirectoryExhausted);
        }
        let segment_index = slot_index / SHARED_VALUE_SEGMENT_SIZE;
        self.ensure_segment(segment_index)
            .ok_or(SharedValueVerdict::DirectoryExhausted)?;
        let slot = self
            .slot(slot_index)
            .ok_or(SharedValueVerdict::DirectoryExhausted)?;
        let generation = slot.activate(initial_value, &self.generation_exhausted)?;
        self.high_water.store(slot_index + 1, Ordering::Release);
        self.live_count.fetch_add(1, Ordering::Relaxed);
        Ok(SharedValueHandle {
            slot: slot_index,
            generation,
            epoch: self.epoch,
        })
    }

    fn validate(&self, handle: SharedValueHandle) -> Result<&SharedValueSlot, SharedValueVerdict> {
        if self.epoch != handle.epoch {
            return Err(SharedValueVerdict::StaleEpoch);
        }
        if handle.slot >= self.high_water.load(Ordering::Acquire) {
            return Err(SharedValueVerdict::StaleSlot);
        }
        let slot = self
            .slot(handle.slot)
            .ok_or(SharedValueVerdict::StaleSlot)?;
        let observed = unpack_generation(slot.tagged_word.load(Ordering::Acquire));
        if is_tombstone(observed) {
            return Err(SharedValueVerdict::Tombstone);
        }
        if observed != handle.generation {
            return Err(SharedValueVerdict::StaleGeneration);
        }
        Ok(slot)
    }

    /// Read a value through a validated handle.
    ///
    /// This is the writer thread's read: it observes the live word directly and
    /// never blocks. Readers on other threads use
    /// [`SharedValueSlab::sample`].
    pub fn read(&self, handle: SharedValueHandle) -> Result<f32, SharedValueVerdict> {
        let slot = self.validate(handle)?;
        Ok(unpack_value(slot.tagged_word.load(Ordering::Acquire)))
    }

    /// Publish a value through a validated handle.
    ///
    /// Refused unless the caller owns the value plane and the handle names the
    /// slot's live tenant.
    pub fn write(&self, handle: SharedValueHandle, value: f32) -> SharedValueVerdict {
        if !self.is_writer_thread() {
            return SharedValueVerdict::WrongWriterThread;
        }
        if !value.is_finite() {
            return SharedValueVerdict::NonFiniteValue;
        }
        match self.validate(handle) {
            Ok(slot) => slot.try_publish(handle.generation, value, &self.generation_exhausted),
            Err(verdict) => verdict,
        }
    }

    /// Retire the tenant named by `handle`. Later operations on that handle are
    /// refused with [`SharedValueVerdict::Tombstone`].
    pub fn free(&self, handle: SharedValueHandle) -> SharedValueVerdict {
        if !self.is_writer_thread() {
            return SharedValueVerdict::WrongWriterThread;
        }
        match self.validate(handle) {
            Ok(slot) => {
                let verdict = slot.tombstone(handle.generation, &self.generation_exhausted);
                if verdict == SharedValueVerdict::Live {
                    self.live_count.fetch_sub(1, Ordering::Relaxed);
                }
                verdict
            }
            Err(verdict) => verdict,
        }
    }

    /// Register a sampler on a value, so it can be read from another thread.
    ///
    /// Registration is what turns on the slot's publication sequence: an
    /// unsampled slot's writes cost one CAS and nothing more.
    pub fn register_sampler(&self, handle: SharedValueHandle) -> SharedValueVerdict {
        match self.validate(handle) {
            Ok(slot) => {
                slot.sampler_count.fetch_add(1, Ordering::AcqRel);
                slot.sampler_sequence_required
                    .store(true, Ordering::Release);
                self.registered_sampler_count.fetch_add(1, Ordering::AcqRel);
                SharedValueVerdict::Live
            }
            Err(verdict) => verdict,
        }
    }

    /// Drop one sampler registration.
    ///
    /// The slot keeps its publication sequence forever: a sampler that read the
    /// old registration count may still be between its two sequence loads.
    pub fn unregister_sampler(&self, handle: SharedValueHandle) -> SharedValueVerdict {
        if self.epoch != handle.epoch {
            return SharedValueVerdict::StaleEpoch;
        }
        // `allocate` never issues a handle carrying the tombstone bit. Refusing
        // one here keeps a fabricated handle out of the match below, where
        // `next_generation` would mask the bit off and let `G | TOMBSTONE`
        // satisfy the fence for the tenant retired at `G + 1`.
        if is_tombstone(handle.generation) {
            return SharedValueVerdict::StaleGeneration;
        }
        if handle.slot >= self.high_water.load(Ordering::Acquire) {
            return SharedValueVerdict::StaleSlot;
        }
        let Some(slot) = self.slot(handle.slot) else {
            return SharedValueVerdict::StaleSlot;
        };
        let observed = unpack_generation(slot.tagged_word.load(Ordering::Acquire));
        // A registration outlives its value: unregistering after the tenant was
        // freed is the ordinary teardown order, so the tombstone that fences
        // exactly this handle's generation still counts as a match.
        let identity_matches = if is_tombstone(observed) {
            next_generation(handle.generation) == Some(generation_number(observed))
        } else {
            observed == handle.generation
        };
        if !identity_matches {
            return SharedValueVerdict::StaleGeneration;
        }
        if slot.sampler_count.load(Ordering::Acquire) == 0 {
            return SharedValueVerdict::SamplerNotRegistered;
        }
        slot.sampler_count.fetch_sub(1, Ordering::Release);
        self.registered_sampler_count
            .fetch_sub(1, Ordering::Release);
        SharedValueVerdict::Live
    }

    /// Read a value as a registered sampler.
    ///
    /// This is the cross-thread read: a seqlock retry loop that never blocks
    /// the writer and never observes a torn word.
    pub fn sample(
        &self,
        handle: SharedValueHandle,
    ) -> Result<SharedValueSample, SharedValueVerdict> {
        if self.epoch != handle.epoch {
            return Err(SharedValueVerdict::StaleEpoch);
        }
        if handle.slot >= self.high_water.load(Ordering::Acquire) {
            return Err(SharedValueVerdict::StaleSlot);
        }
        let slot = self
            .slot(handle.slot)
            .ok_or(SharedValueVerdict::StaleSlot)?;
        // Identity is checked before the seqlock read, the same way `read`
        // checks it, so a handle that names a tenant this slot does not hold
        // is refused rather than served someone else's value. The check is
        // repeated inside the loop's validated word: this one is the cheap
        // rejection, that one is the one the seqlock actually proves.
        let observed = unpack_generation(slot.tagged_word.load(Ordering::Acquire));
        if is_tombstone(observed) {
            return Err(SharedValueVerdict::Tombstone);
        }
        if observed != handle.generation {
            return Err(SharedValueVerdict::StaleGeneration);
        }
        slot.sample(handle.generation)
    }
}

impl Drop for SharedValueSlab {
    fn drop(&mut self) {
        for entry in &self.directory {
            let segment = entry.load(Ordering::Relaxed);
            if !segment.is_null() {
                // SAFETY: destruction requires quiescence; each published
                // segment pointer is unique and visited once.
                drop(unsafe { Box::from_raw(segment) });
            }
        }
    }
}

#[cfg(test)]
#[path = "shared_value_tests.rs"]
mod tests;
