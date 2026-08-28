//! @ref LLP 0099#SharedValue

use super::*;

#[test]
fn allocate_publish_and_read_refuse_stale_identity() {
    let slab = SharedValueSlab::new(4).expect("slab");
    let handle = slab.allocate(1.5).expect("allocate");
    assert_eq!(slab.read(handle), Ok(1.5));
    assert_eq!(slab.live_count(), 1);

    assert_eq!(slab.write(handle, 42.5), SharedValueVerdict::Live);
    assert_eq!(slab.read(handle), Ok(42.5));

    // A stale generation is refused, never silently served.
    let stale = SharedValueHandle {
        generation: handle.generation + 1,
        ..handle
    };
    assert_eq!(slab.read(stale), Err(SharedValueVerdict::StaleGeneration));
    let other_epoch = SharedValueHandle {
        epoch: handle.epoch + 1,
        ..handle
    };
    assert_eq!(slab.read(other_epoch), Err(SharedValueVerdict::StaleEpoch));
    let unallocated = SharedValueHandle { slot: 3, ..handle };
    assert_eq!(slab.read(unallocated), Err(SharedValueVerdict::StaleSlot));
}

#[test]
fn a_freed_slot_is_never_re_tenanted_and_its_handle_stays_refused() {
    let slab = SharedValueSlab::new(2).expect("slab");
    let first = slab.allocate(1.0).expect("allocate");
    assert_eq!(slab.free(first), SharedValueVerdict::Live);
    assert_eq!(slab.read(first), Err(SharedValueVerdict::Tombstone));
    assert_eq!(slab.write(first, 2.0), SharedValueVerdict::Tombstone);
    assert_eq!(slab.free(first), SharedValueVerdict::Tombstone);
    assert_eq!(slab.live_count(), 0);

    let second = slab.allocate(3.0).expect("allocate after free");
    assert_ne!(second.slot, first.slot, "slots are never re-tenanted");
    assert_eq!(slab.read(second), Ok(3.0));
}

#[test]
fn non_finite_values_never_enter_the_value_plane() {
    let slab = SharedValueSlab::new(1).expect("slab");
    assert_eq!(
        slab.allocate(f32::NAN),
        Err(SharedValueVerdict::NonFiniteValue)
    );
    let handle = slab.allocate(0.0).expect("allocate");
    assert_eq!(
        slab.write(handle, f32::INFINITY),
        SharedValueVerdict::NonFiniteValue
    );
    assert_eq!(slab.read(handle), Ok(0.0));
}

#[test]
fn writes_from_another_thread_are_refused_rather_than_raced() {
    let slab = SharedValueSlab::new(1).expect("slab");
    let handle = slab.allocate(7.0).expect("allocate");
    assert!(slab.is_writer_thread());

    std::thread::scope(|scope| {
        scope.spawn(|| {
            assert!(!slab.is_writer_thread());
            assert_eq!(
                slab.write(handle, 9.0),
                SharedValueVerdict::WrongWriterThread
            );
            assert_eq!(
                slab.allocate(1.0),
                Err(SharedValueVerdict::WrongWriterThread)
            );
            assert_eq!(slab.free(handle), SharedValueVerdict::WrongWriterThread);
        });
    });

    assert_eq!(slab.read(handle), Ok(7.0));
}

#[test]
fn a_registered_sampler_reads_across_threads_and_sees_write_generations() {
    let slab = SharedValueSlab::new(1).expect("slab");
    let handle = slab.allocate(1.0).expect("allocate");
    assert_eq!(
        slab.sample(handle),
        Err(SharedValueVerdict::SamplerNotRegistered)
    );

    assert_eq!(slab.register_sampler(handle), SharedValueVerdict::Live);
    assert_eq!(slab.registered_sampler_count(), 1);
    let first = slab.sample(handle).expect("registered sample");
    assert_eq!(first.value, 1.0);

    assert_eq!(slab.write(handle, 2.0), SharedValueVerdict::Live);
    let second = std::thread::scope(|scope| {
        scope
            .spawn(|| slab.sample(handle).expect("cross-thread sample"))
            .join()
            .expect("sampler thread")
    });
    assert_eq!(second.value, 2.0);
    assert!(second.write_generation > first.write_generation);

    assert_eq!(slab.free(handle), SharedValueVerdict::Live);
    assert_eq!(slab.sample(handle), Err(SharedValueVerdict::Tombstone));
    assert_eq!(slab.unregister_sampler(handle), SharedValueVerdict::Live);
    assert_eq!(slab.registered_sampler_count(), 0);
}

#[test]
fn a_fabricated_tombstoned_handle_cannot_release_a_sampler_registration() {
    let slab = SharedValueSlab::new(2).expect("slab");
    let handle = slab.allocate(1.0).expect("allocate");
    assert_eq!(slab.register_sampler(handle), SharedValueVerdict::Live);
    assert_eq!(slab.registered_sampler_count(), 1);

    // `allocate` never issues a handle carrying the tombstone bit. Freeing the
    // tenant moves the slot to `generation + 1 | TOMBSTONE`, which is exactly
    // the value `next_generation` would compute for a fabricated
    // `generation | TOMBSTONE` handle once the bit is masked off.
    assert_eq!(slab.free(handle), SharedValueVerdict::Live);
    let forged = SharedValueHandle {
        generation: handle.generation | SHARED_VALUE_TOMBSTONE_BIT,
        ..handle
    };
    assert_eq!(
        slab.unregister_sampler(forged),
        SharedValueVerdict::StaleGeneration
    );
    assert_eq!(
        slab.registered_sampler_count(),
        1,
        "a forged handle released a registration it never held"
    );

    // The real handle still completes the ordinary teardown order.
    assert_eq!(slab.unregister_sampler(handle), SharedValueVerdict::Live);
    assert_eq!(slab.registered_sampler_count(), 0);
}

#[test]
fn a_sampler_read_validates_the_handle_generation_like_a_read_does() {
    let slab = SharedValueSlab::new(2).expect("slab");
    let handle = slab.allocate(1.0).expect("allocate");
    assert_eq!(slab.register_sampler(handle), SharedValueVerdict::Live);
    assert_eq!(
        slab.sample(handle).expect("sample").value,
        1.0,
        "the live tenant is served"
    );

    // `read` refuses a handle that names a tenant this slot does not hold.
    // `sample` is the cross-thread read of the same word and must agree.
    let stale = SharedValueHandle {
        generation: handle.generation + 1,
        ..handle
    };
    assert_eq!(slab.read(stale), Err(SharedValueVerdict::StaleGeneration));
    assert_eq!(
        slab.sample(stale),
        Err(SharedValueVerdict::StaleGeneration),
        "sample served a value for a generation it was not asked for"
    );

    assert_eq!(slab.free(handle), SharedValueVerdict::Live);
    assert_eq!(slab.sample(handle), Err(SharedValueVerdict::Tombstone));
}
