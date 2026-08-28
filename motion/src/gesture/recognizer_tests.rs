//! @ref LLP 0099#gesture-recognizers
//! @ref LLP 0099#velocity-tracking

use super::*;

fn common(id: u64, min_pointers: u8, max_pointers: u8) -> RecognizerCommon {
    RecognizerCommon {
        id: GestureId(id),
        generation: 1,
        min_pointers,
        max_pointers,
    }
}

fn contact(id: u64, x: f64, y: f64) -> PointerContact {
    PointerContact {
        id: PointerId(id),
        position: MotionPoint { x, y },
        absolute_position: MotionPoint { x, y },
        delta: MotionPoint::ZERO,
        pressure: 0.0,
    }
}

fn frame(
    stream: u64,
    phase: PointerPhase,
    timestamp_ms: f64,
    contacts: &[(u64, f64, f64)],
) -> PointerFrame {
    PointerFrame::new(
        GestureStreamId(stream),
        phase,
        timestamp_ms,
        contacts.iter().map(|(id, x, y)| contact(*id, *x, *y)),
    )
    .expect("valid fixture frame")
}

fn only_event(step: RecognizerStep) -> RecognizerEvent {
    assert_eq!(step.events.len(), 1);
    step.events.into_iter().next().expect("one event")
}

#[test]
fn wire_enums_and_descriptors_reject_invalid_values_and_non_finite_fields() {
    assert!(matches!(
        RecognizerKind::try_from(255),
        Err(GestureDescriptorError::InvalidEnum { .. })
    ));
    assert!(matches!(
        GestureAxis::try_from(3),
        Err(GestureDescriptorError::InvalidEnum { .. })
    ));
    assert!(matches!(
        PointerPhase::try_from(0),
        Err(GestureInputError::InvalidEnum { .. })
    ));
    assert!(matches!(
        ArenaClaimKind::try_from(0),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));
    assert!(matches!(
        ArenaProfileKind::try_from(0),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));
    assert!(matches!(
        ArenaAxis::try_from(0),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));
    assert!(matches!(
        ArenaDirection::try_from(0),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));
    assert!(matches!(
        ArenaInputKind::try_from(0),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));
    assert!(matches!(
        CompoundPolicy::try_from(0),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));
    assert!(matches!(
        ArenaRelationshipKind::try_from(0),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));
    assert!(matches!(
        BackMode::try_from(9),
        Err(ArenaProfileError::InvalidEnum { .. })
    ));

    let invalid = RecognizerDescriptor::Pan(PanDescriptor {
        common: common(1, 1, 1),
        axis: GestureAxis::Horizontal,
        minimum_distance: f64::NAN,
        axis_lock_ratio: 1.25,
        fail_cross_axis: true,
        active_offset_x: None,
        active_offset_y: None,
    });
    assert_eq!(
        invalid.validate(),
        Err(GestureDescriptorError::NonFinite("pan.minimum_distance"))
    );
    // Zero is the authored "activate on first movement" idiom — the web
    // recognizer's semantics — and shipped Contract components author it.
    // It must validate; rejecting it voided whole Motion snapshots.
    let zero_distance = RecognizerDescriptor::Pan(PanDescriptor {
        common: common(1, 1, 1),
        axis: GestureAxis::Horizontal,
        minimum_distance: 0.0,
        axis_lock_ratio: 1.25,
        fail_cross_axis: true,
        active_offset_x: None,
        active_offset_y: None,
    });
    assert!(zero_distance.validate().is_ok());
    let negative_distance = RecognizerDescriptor::Pan(PanDescriptor {
        common: common(1, 1, 1),
        axis: GestureAxis::Horizontal,
        minimum_distance: -1.0,
        axis_lock_ratio: 1.25,
        fail_cross_axis: true,
        active_offset_x: None,
        active_offset_y: None,
    });
    assert_eq!(
        negative_distance.validate(),
        Err(GestureDescriptorError::OutOfRange("pan.minimum_distance"))
    );
    let invalid_pinch = RecognizerDescriptor::Pinch(PinchDescriptor {
        common: common(2, 1, 2),
        minimum_scale_delta: 0.05,
    });
    assert_eq!(
        invalid_pinch.validate(),
        Err(GestureDescriptorError::OutOfRange("pinch.pointer_count"))
    );
    assert!(matches!(
        PointerFrame::new(
            GestureStreamId(1),
            PointerPhase::Down,
            0.0,
            [contact(1, f64::INFINITY, 0.0)]
        ),
        Err(GestureInputError::NonFinite("pointer.contact"))
    ));
    let mut invalid_pressure = contact(1, 0.0, 0.0);
    invalid_pressure.pressure = 1.01;
    assert!(matches!(
        PointerFrame::new(
            GestureStreamId(1),
            PointerPhase::Down,
            0.0,
            [invalid_pressure]
        ),
        Err(GestureInputError::OutOfRange("pointer.pressure"))
    ));
}

#[test]
fn pointer_set_identity_is_order_independent_and_rejects_aliases() {
    let first = PointerSetIdentity::new([PointerId(9), PointerId(3)]).expect("valid set");
    let second = PointerSetIdentity::new([PointerId(3), PointerId(9)]).expect("valid set");
    assert_eq!(first, second);
    assert_eq!(first.members(), &[PointerId(3), PointerId(9)]);
    assert_eq!(
        PointerSetIdentity::new([PointerId(3), PointerId(3)]),
        Err(GestureInputError::DuplicatePointerId)
    );
    assert_eq!(
        PointerSetIdentity::new(Vec::<PointerId>::new()),
        Err(GestureInputError::EmptyPointerSet)
    );
    assert_eq!(
        PointerSetIdentity::new([PointerId(0)]),
        Err(GestureInputError::ZeroIdentity("pointer.id"))
    );
}

#[test]
fn weighted_lsq_velocity_is_capped_horizon_bounded_and_provenanced() {
    assert!(matches!(
        VelocityTracker::new(VELOCITY_HORIZON_MS + 1.0, VELOCITY_SAMPLE_CAP),
        Err(GestureInputError::InvalidHorizon)
    ));
    assert!(matches!(
        VelocityTracker::new(VELOCITY_HORIZON_MS, VELOCITY_SAMPLE_CAP + 1),
        Err(GestureInputError::InvalidCapacity)
    ));
    let pointers = PointerSetIdentity::new([PointerId(1)]).expect("valid set");
    let mut tracker = VelocityTracker::default();
    for index in 0..30 {
        let timestamp_ms = index as f64 * 10.0;
        tracker
            .add_sample(
                &pointers,
                MotionPoint {
                    x: timestamp_ms * 0.1,
                    y: timestamp_ms * -0.2,
                },
                timestamp_ms,
            )
            .expect("monotonic sample");
    }
    let estimate = tracker.estimate();
    assert!((estimate.velocity_per_second.x - 100.0).abs() < 1.0e-6);
    assert!((estimate.velocity_per_second.y + 200.0).abs() < 1.0e-6);
    assert_eq!(
        estimate.provenance.estimator,
        VelocityEstimator::WeightedQuadratic
    );
    assert!(estimate.provenance.sample_count <= VELOCITY_SAMPLE_CAP);
    assert!(estimate.provenance.observed_horizon_ms <= VELOCITY_HORIZON_MS);
    assert_eq!(estimate.provenance.pointer_set, Some(pointers.clone()));

    tracker
        .add_sample(&pointers, MotionPoint { x: 31.0, y: -62.0 }, 290.0)
        .expect("same timestamp replaces sample");
    assert_eq!(
        tracker.add_sample(&pointers, MotionPoint::ZERO, 1.0),
        Err(GestureInputError::TimestampMovedBackward)
    );
    let other = PointerSetIdentity::new([PointerId(2)]).expect("valid set");
    assert_eq!(
        tracker.add_sample(&other, MotionPoint::ZERO, 300.0),
        Err(GestureInputError::StreamChanged)
    );

    let mut after_gap = VelocityTracker::default();
    after_gap
        .add_sample(&pointers, MotionPoint::ZERO, 0.0)
        .expect("first sample");
    after_gap
        .add_sample(&pointers, MotionPoint { x: 100.0, y: 0.0 }, 200.0)
        .expect("gap sample");
    assert_eq!(
        after_gap.estimate().provenance.estimator,
        VelocityEstimator::InsufficientSamples
    );
}

#[test]
fn pan_tracks_velocity_and_pointer_membership_loss_is_phase_dependent() {
    let descriptor = RecognizerDescriptor::Pan(PanDescriptor {
        common: common(1, 1, 2),
        axis: GestureAxis::Horizontal,
        minimum_distance: 12.0,
        axis_lock_ratio: 1.25,
        fail_cross_axis: true,
        active_offset_x: None,
        active_offset_y: None,
    });
    let mut pan = GestureRecognizer::new(descriptor).expect("valid descriptor");
    assert_eq!(
        only_event(
            pan.process(&frame(1, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
                .expect("begin")
        )
        .state,
        RecognizerState::Began
    );
    let active = only_event(
        pan.process(&frame(1, PointerPhase::Move, 16.0, &[(1, 20.0, 1.0)]))
            .expect("activation"),
    );
    assert_eq!(active.state, RecognizerState::Active);
    let GesturePayload::Pan { velocity, .. } = active.payload else {
        panic!("expected pan payload")
    };
    assert!(velocity.velocity_per_second.x > 1_000.0);
    let cancelled = only_event(
        pan.process(&frame(
            1,
            PointerPhase::Move,
            32.0,
            &[(1, 30.0, 1.0), (2, 30.0, 1.0)],
        ))
        .expect("membership cancellation"),
    );
    assert_eq!(cancelled.state, RecognizerState::Cancelled);
    assert_eq!(
        cancelled.terminal_reason,
        Some(GestureTerminalReason::PointerSetChanged)
    );

    pan.reset(2).expect("new generation");
    pan.process(&frame(2, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("begin");
    let failed = only_event(
        pan.process(&frame(
            2,
            PointerPhase::Move,
            5.0,
            &[(1, 1.0, 0.0), (2, 1.0, 0.0)],
        ))
        .expect("preactivation membership failure"),
    );
    assert_eq!(failed.state, RecognizerState::Failed);
}

#[test]
fn two_pointer_recognizer_joins_members_without_restarting_stream() {
    let mut pinch = GestureRecognizer::new(RecognizerDescriptor::Pinch(PinchDescriptor {
        common: common(7, 2, 2),
        minimum_scale_delta: 0.1,
    }))
    .expect("valid pinch");

    let waiting = pinch
        .process(&frame(41, PointerPhase::Down, 0.0, &[(17, -5.0, 0.0)]))
        .expect("first member starts the arena stream");
    assert!(waiting.events.is_empty());
    assert_eq!(pinch.state(), RecognizerState::Began);
    assert!(pinch
        .process(&frame(41, PointerPhase::Move, 0.5, &[(17, -4.0, 0.0)]))
        .expect("one-member movement remains possible")
        .events
        .is_empty());

    let joined = only_event(
        pinch
            .process(&frame(
                41,
                PointerPhase::Down,
                1.0,
                &[(17, -5.0, 0.0), (23, 5.0, 0.0)],
            ))
            .expect("second stable member joins"),
    );
    assert_eq!(joined.stream_id, GestureStreamId(41));
    assert_eq!(
        joined.pointer_set.members(),
        &[PointerId(17), PointerId(23)]
    );

    let active = only_event(
        pinch
            .process(&frame(
                41,
                PointerPhase::Move,
                20.0,
                &[(17, -6.0, 0.0), (23, 6.0, 0.0)],
            ))
            .expect("joined members activate"),
    );
    assert_eq!(active.state, RecognizerState::Active);
    let ended = only_event(
        pinch
            .process(&frame(
                41,
                PointerPhase::Up,
                30.0,
                &[(17, -6.0, 0.0), (23, 6.0, 0.0)],
            ))
            .expect("terminal membership is complete"),
    );
    assert_eq!(ended.state, RecognizerState::Ended);
    assert_eq!(ended.pointer_set.members(), &[PointerId(17), PointerId(23)]);
}

#[test]
fn pan_cross_axis_fails_before_activation_and_cancel_after_activation_is_once() {
    let mut pan = GestureRecognizer::new(RecognizerDescriptor::Pan(PanDescriptor {
        common: common(3, 1, 1),
        axis: GestureAxis::Horizontal,
        minimum_distance: 10.0,
        axis_lock_ratio: 1.25,
        fail_cross_axis: true,
        active_offset_x: None,
        active_offset_y: None,
    }))
    .expect("valid pan");
    pan.process(&frame(1, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("begin");
    let failed = only_event(
        pan.process(&frame(1, PointerPhase::Move, 10.0, &[(1, 1.0, 20.0)]))
            .expect("cross-axis fail"),
    );
    assert_eq!(failed.state, RecognizerState::Failed);
    assert_eq!(
        pan.process(&frame(1, PointerPhase::Up, 20.0, &[(1, 1.0, 20.0)])),
        Err(GestureInputError::TerminalState)
    );

    pan.reset(2).expect("rearm");
    pan.process(&frame(2, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("begin");
    pan.process(&frame(2, PointerPhase::Move, 10.0, &[(1, 20.0, 0.0)]))
        .expect("activate");
    let cancelled = only_event(
        pan.cancel(GestureTerminalReason::ExternalOwner, 11.0)
            .expect("cancel"),
    );
    assert_eq!(cancelled.state, RecognizerState::Cancelled);
    assert!(pan
        .cancel(GestureTerminalReason::ExternalOwner, 12.0)
        .expect("idempotent terminal")
        .events
        .is_empty());
}

#[test]
fn pan_active_offsets_replace_distance_activation() {
    let mut pan = GestureRecognizer::new(RecognizerDescriptor::Pan(PanDescriptor {
        common: common(4, 1, 1),
        axis: GestureAxis::Horizontal,
        minimum_distance: 10.0,
        axis_lock_ratio: 1.25,
        fail_cross_axis: false,
        active_offset_x: Some((-100.0, 100.0)),
        active_offset_y: None,
    }))
    .expect("valid pan");
    pan.process(&frame(1, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("begin");
    assert!(pan
        .process(&frame(1, PointerPhase::Move, 10.0, &[(1, 20.0, 0.0)]))
        .expect("inside offset remains possible")
        .events
        .is_empty());
    let active = only_event(
        pan.process(&frame(1, PointerPhase::Move, 20.0, &[(1, 101.0, 0.0)]))
            .expect("outside offset activates"),
    );
    assert_eq!(active.state, RecognizerState::Active);
}

#[test]
fn tap_and_long_press_have_deterministic_time_and_distance_terminals() {
    let mut tap = GestureRecognizer::new(RecognizerDescriptor::Tap(TapDescriptor {
        common: common(4, 1, 1),
        maximum_duration_ms: 250.0,
        maximum_inter_tap_ms: 300.0,
        maximum_distance: 8.0,
        tap_count: 2,
    }))
    .expect("valid tap");
    tap.process(&frame(1, PointerPhase::Down, 0.0, &[(1, 5.0, 6.0)]))
        .expect("begin");
    let first_tap = only_event(
        tap.process(&frame(1, PointerPhase::Up, 100.0, &[(1, 6.0, 6.0)]))
            .expect("first tap"),
    );
    assert_eq!(first_tap.state, RecognizerState::Began);
    assert!(matches!(
        first_tap.payload,
        GesturePayload::Tap { tap_count: 1, .. }
    ));
    assert!(tap
        .process(&frame(2, PointerPhase::Down, 150.0, &[(2, 5.0, 6.0)]))
        .expect("second down")
        .events
        .is_empty());
    let ended = only_event(
        tap.process(&frame(2, PointerPhase::Up, 200.0, &[(2, 6.0, 6.0)]))
            .expect("second tap"),
    );
    assert_eq!(ended.state, RecognizerState::Ended);
    assert!(matches!(
        ended.payload,
        GesturePayload::Tap { tap_count: 2, .. }
    ));

    let mut late_tap = GestureRecognizer::new(RecognizerDescriptor::Tap(TapDescriptor {
        common: common(40, 1, 1),
        maximum_duration_ms: 250.0,
        maximum_inter_tap_ms: 300.0,
        maximum_distance: 8.0,
        tap_count: 2,
    }))
    .expect("valid tap");
    late_tap
        .process(&frame(10, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("first down");
    late_tap
        .process(&frame(10, PointerPhase::Up, 10.0, &[(1, 0.0, 0.0)]))
        .expect("first up");
    let failed = only_event(
        late_tap
            .process(&frame(11, PointerPhase::Down, 311.0, &[(2, 0.0, 0.0)]))
            .expect("inter-tap timeout"),
    );
    assert_eq!(failed.state, RecognizerState::Failed);
    assert_eq!(failed.terminal_reason, Some(GestureTerminalReason::TooLong));

    let mut long = GestureRecognizer::new(RecognizerDescriptor::LongPress(LongPressDescriptor {
        common: common(5, 1, 1),
        minimum_duration_ms: 500.0,
        maximum_distance: 8.0,
    }))
    .expect("valid long press");
    long.process(&frame(2, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("begin");
    let began = only_event(
        long.process(&frame(2, PointerPhase::Timer, 500.0, &[(1, 0.0, 0.0)]))
            .expect("timer activation"),
    );
    assert_eq!(began.state, RecognizerState::Active);
    let ended = only_event(
        long.process(&frame(2, PointerPhase::Up, 550.0, &[(1, 0.0, 0.0)]))
            .expect("end"),
    );
    assert_eq!(ended.state, RecognizerState::Ended);
}

#[test]
fn fling_pinch_and_rotation_emit_typed_terminal_payloads() {
    let mut fling = GestureRecognizer::new(RecognizerDescriptor::Fling(FlingDescriptor {
        common: common(6, 1, 1),
        axis: GestureAxis::Horizontal,
        minimum_velocity_per_second: 500.0,
        minimum_distance: 8.0,
    }))
    .expect("valid fling");
    fling
        .process(&frame(1, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("begin");
    fling
        .process(&frame(1, PointerPhase::Move, 10.0, &[(1, 10.0, 0.0)]))
        .expect("move");
    let fling_event = only_event(
        fling
            .process(&frame(1, PointerPhase::Up, 20.0, &[(1, 30.0, 0.0)]))
            .expect("fling"),
    );
    assert!(matches!(fling_event.payload, GesturePayload::Fling { .. }));

    let mut pinch = GestureRecognizer::new(RecognizerDescriptor::Pinch(PinchDescriptor {
        common: common(7, 2, 2),
        minimum_scale_delta: 0.1,
    }))
    .expect("valid pinch");
    pinch
        .process(&frame(
            2,
            PointerPhase::Down,
            0.0,
            &[(1, -5.0, 0.0), (2, 5.0, 0.0)],
        ))
        .expect("begin");
    let changed = only_event(
        pinch
            .process(&frame(
                2,
                PointerPhase::Move,
                20.0,
                &[(1, -6.0, 0.0), (2, 6.0, 0.0)],
            ))
            .expect("pinch"),
    );
    assert!(matches!(
        changed.payload,
        GesturePayload::Pinch { scale, .. } if (scale - 1.2).abs() < 1.0e-9
    ));
    let ended = only_event(
        pinch
            .process(&frame(
                2,
                PointerPhase::Up,
                30.0,
                &[(1, -6.0, 0.0), (2, 6.0, 0.0)],
            ))
            .expect("pinch end"),
    );
    assert_eq!(ended.state, RecognizerState::Ended);

    let mut rotation = GestureRecognizer::new(RecognizerDescriptor::Rotation(RotationDescriptor {
        common: common(8, 2, 2),
        minimum_rotation_radians: 0.2,
    }))
    .expect("valid rotation");
    rotation
        .process(&frame(
            3,
            PointerPhase::Down,
            0.0,
            &[(1, -5.0, 0.0), (2, 5.0, 0.0)],
        ))
        .expect("begin");
    let changed = only_event(
        rotation
            .process(&frame(
                3,
                PointerPhase::Move,
                20.0,
                &[(1, 0.0, -5.0), (2, 0.0, 5.0)],
            ))
            .expect("rotate"),
    );
    assert!(matches!(
        changed.payload,
        GesturePayload::Rotation { radians, .. }
            if (radians - std::f64::consts::FRAC_PI_2).abs() < 1.0e-9
    ));
}

#[test]
fn reset_fences_inflight_events_and_rearms_generation() {
    let mut recognizer = GestureRecognizer::new(RecognizerDescriptor::Pan(PanDescriptor {
        common: common(9, 1, 1),
        axis: GestureAxis::Any,
        minimum_distance: 1.0,
        axis_lock_ratio: 1.0,
        fail_cross_axis: false,
        active_offset_x: None,
        active_offset_y: None,
    }))
    .expect("valid pan");
    recognizer
        .process(&frame(1, PointerPhase::Down, 0.0, &[(1, 0.0, 0.0)]))
        .expect("begin");
    recognizer
        .process(&frame(1, PointerPhase::Move, 1.0, &[(1, 2.0, 0.0)]))
        .expect("active");
    recognizer.reset(44).expect("reset fence");
    assert_eq!(recognizer.state(), RecognizerState::Idle);
    let event = only_event(
        recognizer
            .process(&frame(2, PointerPhase::Down, 2.0, &[(1, 0.0, 0.0)]))
            .expect("new stream"),
    );
    assert_eq!(event.descriptor_generation, 44);
    assert_eq!(
        recognizer.reset(0),
        Err(GestureDescriptorError::ZeroIdentity(
            "recognizer.generation"
        ))
    );
}
