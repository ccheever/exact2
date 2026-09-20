//! Read readiness is deterministic: scripted syscalls, the actual event path.
use super::*;
use drm::control::PageFlipEvent;
use std::cell::Cell;
use std::collections::VecDeque;
use std::io::{self, ErrorKind};
use std::num::NonZeroU32;
use std::sync::Arc;

fn crtc(id: u32) -> crtc::Handle {
    NonZeroU32::new(id).unwrap().into()
}
fn flip(crtc_id: u32, sequence: u32) -> Event {
    Event::PageFlip(PageFlipEvent {
        frame: sequence,
        duration: Duration::from_millis(16),
        crtc: crtc(crtc_id),
    })
}
fn read_once(last: Option<u32>, result: io::Result<Vec<Event>>) -> io::Result<Option<u32>> {
    let mut result = Some(result);
    receive_flip(crtc(7), last, || {
        result
            .take()
            .expect("one readable turn must perform at most one bounded DRM read")
    })
}

#[test]
fn matching_crtc_read_accepts_the_frame_sequence() {
    assert_eq!(read_once(None, Ok(vec![flip(7, 30)])).unwrap(), Some(30));
}

#[test]
fn wrong_crtc_does_not_complete_a_submitted_frame_or_request_another_read() {
    assert_eq!(read_once(None, Ok(vec![flip(8, 30)])).unwrap(), None);
}

#[test]
fn nonblocking_empty_and_interrupted_reads_leave_the_flip_pending() {
    for kind in [ErrorKind::WouldBlock, ErrorKind::Interrupted] {
        assert_eq!(read_once(None, Err(io::Error::from(kind))).unwrap(), None);
    }
}

#[test]
fn empty_event_batch_yields_to_existing_input_worker_and_timer_processing() {
    let reads = Cell::new(0);
    let mut scheduled = VecDeque::from([Ok(Vec::new()), Ok(vec![flip(7, 3)])]);
    let result = receive_flip(crtc(7), None, || {
        reads.set(reads.get() + 1);
        scheduled.pop_front().expect("bounded scripted events")
    })
    .unwrap();
    assert_eq!(
        result, None,
        "a readiness turn must not wait for a later flip"
    );
    assert_eq!(
        reads.get(),
        1,
        "leave the next read to the next FD-ready turn"
    );
    assert_eq!(scheduled.len(), 1);
}

#[test]
fn duplicate_and_older_sequences_cannot_retire_a_successor() {
    for sequence in [49, 50] {
        assert_eq!(
            read_once(Some(50), Ok(vec![flip(7, sequence)])).unwrap(),
            None
        );
    }
    assert_eq!(
        read_once(Some(50), Ok(vec![flip(7, 51)])).unwrap(),
        Some(51)
    );
}

#[test]
fn crtc_sequence_wrap_is_forward_but_half_range_ambiguity_is_not() {
    assert_eq!(
        read_once(Some(u32::MAX), Ok(vec![flip(7, 0)])).unwrap(),
        Some(0)
    );
    assert_eq!(
        read_once(Some(0), Ok(vec![flip(7, 1 << 31)])).unwrap(),
        None
    );
}

#[test]
fn read_errors_propagate_without_retry() {
    let error = read_once(Some(50), Err(io::Error::from(ErrorKind::BrokenPipe))).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::BrokenPipe);
}

fn frame(value: u8) -> crate::presenter::SubmittedFrame {
    crate::presenter::SubmittedFrame::fixture(value)
}

#[test]
fn two_buffers_have_one_pending_owner_and_neither_front_nor_pending_is_overwritten() {
    let mut slots = FlipState::default();
    let mut writes = Vec::new();
    let first = slots
        .submit(frame(1), |back, first, _| {
            writes.push((back, first));
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(slots.front, 1);
    assert!(!slots.pending());
    assert_eq!(first.pixels.data()[0], 1);
    assert!(slots
        .submit(frame(2), |back, first, _| {
            writes.push((back, first));
            Ok(())
        })
        .unwrap()
        .is_none());
    assert_eq!(slots.front, 1);
    assert!(slots.pending());
    assert!(slots
        .submit(frame(99), |_, _, _| panic!(
            "occupied slot must refuse BEFORE copy"
        ))
        .is_err());
    assert_eq!(writes, [(1, true), (0, false)]);
    let completed = slots
        .ready(crtc(7), || Ok(vec![flip(7, 21)]))
        .unwrap()
        .unwrap();
    assert_eq!(completed.pixels.data()[0], 2);
    assert_eq!(slots.front, 0);
    assert!(!slots.pending());
    slots
        .submit(frame(3), |back, first, _| {
            writes.push((back, first));
            Ok(())
        })
        .unwrap();
    assert_eq!(writes, [(1, true), (0, false), (1, false)]);
}

#[test]
fn copy_or_ioctl_failure_does_not_change_front_or_claim_a_submission() {
    let mut slots = FlipState::default();
    assert!(slots
        .submit(frame(1), |_, _, _| Err("set_crtc refusal".into()))
        .is_err());
    assert!(slots.first);
    assert_eq!(slots.front, 0);
    assert!(!slots.pending());
    slots.submit(frame(1), |_, _, _| Ok(())).unwrap();
    let rejected = frame(2);
    let weak = Arc::downgrade(&rejected.pixels);
    assert!(slots
        .submit(rejected, |back, first, _| {
            assert_eq!(back, 0);
            assert!(!first);
            Err("page_flip refusal after copy".into())
        })
        .is_err());
    assert!(weak.upgrade().is_none());
    assert_eq!(slots.front, 1);
    assert!(!slots.pending());
}

#[test]
fn no_event_wrong_crtc_and_read_errors_retain_the_exact_pending_pixels() {
    let mut slots = FlipState::default();
    slots.submit(frame(1), |_, _, _| Ok(())).unwrap();
    let submitted = frame(2);
    let weak = Arc::downgrade(&submitted.pixels);
    slots.submit(submitted, |_, _, _| Ok(())).unwrap();
    for read in [
        Ok(Vec::new()),
        Ok(vec![flip(8, 21)]),
        Err(io::Error::from(ErrorKind::WouldBlock)),
        Err(io::Error::from(ErrorKind::Interrupted)),
    ] {
        assert!(slots.ready(crtc(7), || read).unwrap().is_none());
        assert!(slots.pending());
        assert_eq!(slots.front, 1);
        assert_eq!(weak.upgrade().unwrap().data()[0], 2);
    }
    assert!(slots
        .ready(crtc(7), || Err(io::Error::from(ErrorKind::BrokenPipe)))
        .is_err());
    assert!(slots.pending());
    drop(slots);
    assert!(
        weak.upgrade().is_none(),
        "shutdown must not retain a pixmap history"
    );
}

#[test]
fn duplicate_event_does_not_release_a_new_submission() {
    let mut slots = FlipState::default();
    slots.submit(frame(1), |_, _, _| Ok(())).unwrap();
    slots.submit(frame(2), |_, _, _| Ok(())).unwrap();
    slots
        .ready(crtc(7), || Ok(vec![flip(7, 40), flip(7, 40)]))
        .unwrap()
        .unwrap();
    slots.submit(frame(3), |_, _, _| Ok(())).unwrap();
    assert!(slots
        .ready(crtc(7), || Ok(vec![flip(7, 40)]))
        .unwrap()
        .is_none());
    assert_eq!(slots.front, 0);
    assert!(slots.pending());
    let third = slots
        .ready(crtc(7), || Ok(vec![flip(7, 41)]))
        .unwrap()
        .unwrap();
    assert_eq!(third.pixels.data()[0], 3);
    assert!(slots
        .ready(crtc(7), || Ok(vec![flip(7, 41)]))
        .unwrap()
        .is_none());
}

#[test]
fn pending_animation_does_not_spin_but_real_timer_deadlines_remain() {
    assert_eq!(work_timeout(true, true, None, 0., 100., 1000. / 60.), -1);
    assert_eq!(
        work_timeout(true, true, Some(250.), 0., 100., 1000. / 60.),
        150
    );
    assert_eq!(
        work_timeout(true, true, Some(250.), 0., 250., 1000. / 60.),
        0
    );
    assert_eq!(work_timeout(false, true, None, 0., 100., 1000. / 60.), 0);
    assert_eq!(
        work_timeout(false, false, Some(250.), 0., 100., 1000. / 60.),
        150
    );
    assert_eq!(work_timeout(false, false, None, 0., 100., 1000. / 60.), -1);
}

#[test]
fn poll_distinguishes_input_readiness_drm_readiness_and_invalid_fd() {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    let (input, mut input_signal) = UnixStream::pair().unwrap();
    let (drm, mut drm_signal) = UnixStream::pair().unwrap();
    let fds = [input.as_raw_fd(), drm.as_raw_fd()];
    assert!(!poll(&fds, drm.as_raw_fd(), 0).unwrap());
    input_signal.write_all(&[1]).unwrap();
    assert!(
        !poll(&fds, drm.as_raw_fd(), 0).unwrap(),
        "input wake can make progress without completing a flip"
    );
    drm_signal.write_all(&[1]).unwrap();
    assert!(poll(&fds, drm.as_raw_fd(), 0).unwrap());
    assert!(poll(&[i32::MAX], i32::MAX, 0).is_err());
    assert!(
        poll(&[i32::MAX, input.as_raw_fd()], input.as_raw_fd(), 0).is_ok(),
        "other FD lifetime policy is unchanged"
    );
}

#[test]
fn copy_xrgb_fixed_nonopaque_and_transparent_pixels_force_x() {
    let frame = Pixmap::from_vec(
        vec![1, 7, 99, 100, 0, 0, 0, 0, 21, 42, 63, 255],
        tiny_skia::IntSize::from_wh(3, 1).unwrap(),
    )
    .unwrap();
    let before = frame.data().to_vec();
    let mut dst = [165; 16];
    copy_xrgb(&frame, &mut dst[1..14], 13, 3, 1);
    assert_eq!(
        dst,
        [165, 99, 7, 1, 255, 0, 0, 0, 255, 63, 42, 21, 255, 165, 165, 165]
    );
    assert_eq!(frame.data(), before.as_slice());
}

#[test]
fn copy_xrgb_fixed_crop_source_stride_and_odd_destination_pitch() {
    let frame = Pixmap::from_vec(
        (1u8..=60).collect(),
        tiny_skia::IntSize::from_wh(5, 3).unwrap(),
    )
    .unwrap();
    let before = frame.data().to_vec();
    let mut dst = [165; 54];
    let mut expected = [165; 54];
    expected[1..13].copy_from_slice(&[3, 2, 1, 255, 7, 6, 5, 255, 11, 10, 9, 255]);
    expected[18..30].copy_from_slice(&[23, 22, 21, 255, 27, 26, 25, 255, 31, 30, 29, 255]);
    copy_xrgb(&frame, &mut dst[1..52], 17, 3, 2);
    assert_eq!(
        dst, expected,
        "prefix, row padding and uncopied bottom/suffix stay intact"
    );
    assert_eq!(frame.data(), before.as_slice());
}

#[test]
fn copy_xrgb_fixed_seventeen_distinct_pixels_preserve_tail_and_guards() {
    let frame = Pixmap::from_vec(
        (1u8..=68).collect(),
        tiny_skia::IntSize::from_wh(17, 1).unwrap(),
    )
    .unwrap();
    let before = frame.data().to_vec();
    let mut dst = [165; 74];
    let mut expected = [165; 74];
    expected[1..69].copy_from_slice(&[
        3, 2, 1, 255, 7, 6, 5, 255, 11, 10, 9, 255, 15, 14, 13, 255, 19, 18, 17, 255, 23, 22, 21,
        255, 27, 26, 25, 255, 31, 30, 29, 255, 35, 34, 33, 255, 39, 38, 37, 255, 43, 42, 41, 255,
        47, 46, 45, 255, 51, 50, 49, 255, 55, 54, 53, 255, 59, 58, 57, 255, 63, 62, 61, 255, 67,
        66, 65, 255,
    ]);
    copy_xrgb(&frame, &mut dst[1..72], 71, 17, 1);
    assert_eq!(dst, expected);
    assert_eq!(frame.data(), before.as_slice());
}

#[test]
fn copy_xrgb_fixed_min_dimensions_and_zero_extents() {
    let frame =
        Pixmap::from_vec(vec![3, 5, 7, 8], tiny_skia::IntSize::from_wh(1, 1).unwrap()).unwrap();
    for (width, height) in [(0, 1), (1, 0), (0, 0)] {
        let mut dst = [165; 16];
        copy_xrgb(&frame, &mut dst[1..15], 9, width, height);
        assert_eq!(dst, [165; 16]);
    }
    let mut dst = [165; 16];
    copy_xrgb(&frame, &mut dst[1..15], 9, 3, 4);
    assert_eq!(
        dst,
        [165, 7, 5, 3, 255, 165, 165, 165, 165, 165, 165, 165, 165, 165, 165, 165]
    );
    assert_eq!(frame.data(), [3, 5, 7, 8]);
}

#[test]
fn copy_xrgb_fixed_in_bounds_overlapping_pitch_keeps_row_order() {
    let frame = Pixmap::from_vec(
        (1u8..=16).collect(),
        tiny_skia::IntSize::from_wh(2, 2).unwrap(),
    )
    .unwrap();
    let before = frame.data().to_vec();
    let mut dst = [165; 14];
    copy_xrgb(&frame, &mut dst[1..13], 3, 2, 2);
    assert_eq!(
        dst,
        [165, 3, 2, 1, 11, 10, 9, 255, 15, 14, 13, 255, 165, 165]
    );
    assert_eq!(frame.data(), before.as_slice());
}
