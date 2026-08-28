//! @ref LLP 0099#gesture-composition

use super::*;

fn source() -> InteractionSourceIdentity {
    InteractionSourceIdentity {
        epoch: 7,
        root_id: 8,
        root_instance: 9,
        motion_seq: 10,
        profile_generation: 11,
        modal_generation: 12,
        presenter_generation: 13,
        back_structural_revision: 14,
        back_policy_revision: 15,
    }
}

fn claim(
    id: u64,
    kind: ArenaClaimKind,
    depth: u16,
    priority: i16,
    order: u16,
) -> ArenaClaimDescriptor {
    ArenaClaimDescriptor {
        id: ArenaClaimId(id),
        gesture_id: Some(GestureId(id + 100)),
        kind,
        axis: ArenaAxis::Horizontal,
        direction: ArenaDirection::Negative,
        depth,
        priority,
        declaration_order: order,
        compound: None,
    }
}

fn eligibility(
    source: InteractionSourceIdentity,
    back_mode: BackMode,
    claims: &[(u64, bool, bool)],
) -> ArenaEligibilitySnapshot {
    ArenaEligibilitySnapshot {
        source,
        back_mode,
        claims: claims
            .iter()
            .map(|(id, can_consume, scene_lease_acquired)| {
                (
                    ArenaClaimId(*id),
                    ClaimEligibility {
                        can_consume: *can_consume,
                        scene_lease_acquired: *scene_lease_acquired,
                    },
                )
            })
            .collect(),
    }
}

#[test]
fn profile_validation_rejects_cycles_contradictions_and_bad_compounds() {
    let claims = [
        claim(1, ArenaClaimKind::Gesture, 1, 0, 0),
        claim(2, ArenaClaimKind::Gesture, 1, 0, 1),
    ];
    assert!(matches!(
        ArenaProfile::new(
            ArenaProfileKind::IosTouch,
            claims.clone(),
            [],
            [
                ArenaRelationship::RequireFailure {
                    claimant: ArenaClaimId(1),
                    required: ArenaClaimId(2),
                },
                ArenaRelationship::RequireFailure {
                    claimant: ArenaClaimId(2),
                    required: ArenaClaimId(1),
                },
            ],
        ),
        Err(ArenaProfileError::RequireFailureCycle)
    ));
    assert!(matches!(
        ArenaProfile::new(
            ArenaProfileKind::IosTouch,
            claims.clone(),
            [],
            [ArenaRelationship::Simultaneous {
                first: ArenaClaimId(1),
                second: ArenaClaimId(2),
            }],
        ),
        Err(ArenaProfileError::SimultaneousOutsideCompound)
    ));
    assert!(matches!(
        ArenaProfile::new(
            ArenaProfileKind::IosTouch,
            claims,
            [],
            [
                ArenaRelationship::Exclusive {
                    preferred: ArenaClaimId(1),
                    fallback: ArenaClaimId(2),
                },
                ArenaRelationship::Exclusive {
                    preferred: ArenaClaimId(2),
                    fallback: ArenaClaimId(1),
                },
            ],
        ),
        Err(ArenaProfileError::ContradictoryRelationship)
    ));

    let mut first = claim(1, ArenaClaimKind::Gesture, 1, 0, 0);
    first.compound = Some(CompoundClaimId(100));
    let mut second = claim(2, ArenaClaimKind::Gesture, 1, 0, 1);
    second.compound = Some(CompoundClaimId(100));
    assert!(matches!(
        ArenaProfile::new(
            ArenaProfileKind::IosTouch,
            [first, second],
            [CompoundClaimDescriptor {
                id: CompoundClaimId(100),
                policy: CompoundPolicy::Simultaneous,
                members: vec![ArenaClaimId(1), ArenaClaimId(2)],
            }],
            [],
        ),
        Err(ArenaProfileError::IncompleteSimultaneousCompound(
            CompoundClaimId(100)
        ))
    ));
}

#[test]
fn split_publisher_state_store_joins_focus_scroll_and_topology_generations() {
    let profile = ArenaProfile::new(
        ArenaProfileKind::MacosTrackpad,
        [
            claim(1, ArenaClaimKind::Pager, 2, 0, 0),
            claim(2, ArenaClaimKind::Scroll, 3, 0, 1),
        ],
        [],
        [],
    )
    .expect("valid profile");
    let mut store = InteractionStateStore::default();
    store
        .attach_topology(InteractionTopologySnapshot {
            source: source(),
            mounted_claims: [ArenaClaimId(1), ArenaClaimId(2)].into_iter().collect(),
            modal_barrier_id: None,
            declared_layout_direction: LayoutDirection::LeftToRight,
            back_mode: BackMode::None,
        })
        .expect("runtime topology publisher");
    store
        .publish_environment(InteractionEnvironmentSnapshot {
            root_id: 8,
            presenter_generation: 20,
            focused_root_id: Some(8),
            window_id: Some(99),
            effective_layout_direction: LayoutDirection::LeftToRight,
            display_mode: InteractionDisplayMode::Desktop,
            host_chrome_present: true,
        })
        .expect("host environment publisher");
    store
        .publish_scroll_capacity(ScrollCapacityRecord {
            scroll_id: 2,
            root_id: 8,
            axis: ArenaAxis::Horizontal,
            offset: 10.0,
            extent: 100.0,
            viewport: 50.0,
            can_consume_negative: true,
            can_consume_positive: false,
            presenter_generation: 20,
        })
        .expect("presenter scroll publisher");
    assert_eq!(
        store.publish_scroll_capacity(ScrollCapacityRecord {
            scroll_id: 3,
            root_id: 8,
            axis: ArenaAxis::Horizontal,
            offset: f64::NAN,
            extent: 100.0,
            viewport: 50.0,
            can_consume_negative: true,
            can_consume_positive: false,
            presenter_generation: 20,
        }),
        Err(InteractionStateError::InvalidScrollRecord)
    );
    let base = [
        (
            ArenaClaimId(1),
            ClaimEligibility {
                can_consume: true,
                scene_lease_acquired: false,
            },
        ),
        (
            ArenaClaimId(2),
            ClaimEligibility {
                can_consume: true,
                scene_lease_acquired: false,
            },
        ),
    ]
    .into_iter()
    .collect();
    let joined = store
        .arena_snapshot(8, &profile, &base)
        .expect("joined state");
    assert!(joined.claims[&ArenaClaimId(1)].can_consume);
    assert!(joined.claims[&ArenaClaimId(2)].can_consume);
    assert_eq!(joined.source.presenter_generation, 20);

    store
        .publish_environment(InteractionEnvironmentSnapshot {
            root_id: 8,
            presenter_generation: 21,
            focused_root_id: None,
            window_id: None,
            effective_layout_direction: LayoutDirection::LeftToRight,
            display_mode: InteractionDisplayMode::Desktop,
            host_chrome_present: true,
        })
        .expect("focus loss");
    let unfocused = store
        .arena_snapshot(8, &profile, &base)
        .expect("joined state");
    assert!(unfocused.claims.values().all(|claim| !claim.can_consume));
    assert!(store.tombstone_scroll_capacity(2));
    assert!(store.detach_topology(8, 9).expect("matching root"));
    assert!(matches!(
        store.arena_snapshot(8, &profile, &base),
        Err(InteractionStateError::UnknownRoot(8))
    ));
}

#[test]
fn require_failure_and_exclusive_fallback_are_arrival_order_independent() {
    let claims = [
        claim(1, ArenaClaimKind::Gesture, 2, 0, 0),
        claim(2, ArenaClaimKind::Gesture, 1, 0, 1),
    ];
    let profile = ArenaProfile::new(
        ArenaProfileKind::IosTouch,
        claims,
        [],
        [
            ArenaRelationship::RequireFailure {
                claimant: ArenaClaimId(2),
                required: ArenaClaimId(1),
            },
            ArenaRelationship::Exclusive {
                preferred: ArenaClaimId(1),
                fallback: ArenaClaimId(2),
            },
        ],
    )
    .expect("valid profile");
    let snapshot = eligibility(
        source(),
        BackMode::None,
        &[(1, true, false), (2, true, false)],
    );
    let mut arena = GestureArena::new(profile.clone());
    arena
        .begin_stream(
            GestureStreamId(1),
            ArenaInputKind::Touch,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin");
    let pending = arena
        .activate(GestureStreamId(1), [ArenaClaimId(2)], &snapshot)
        .expect("pending resolution");
    assert_eq!(pending.reason, ArenaDecisionReason::RequireFailurePending);
    arena
        .fail_claim(
            GestureStreamId(1),
            ArenaClaimId(1),
            ClaimFailureReason::Ineligible,
        )
        .expect("preferred failed");
    let fallback = arena
        .activate(GestureStreamId(1), [ArenaClaimId(2)], &snapshot)
        .expect("fallback activates");
    assert_eq!(fallback.holder, Some(LeaseHolder::Claim(ArenaClaimId(2))));
}

#[test]
fn simultaneous_and_exclusive_children_hold_one_external_compound_lease() {
    let mut first = claim(1, ArenaClaimKind::Gesture, 2, 0, 0);
    first.compound = Some(CompoundClaimId(100));
    let mut second = claim(2, ArenaClaimKind::Gesture, 2, 0, 1);
    second.compound = Some(CompoundClaimId(100));
    let simultaneous_profile = ArenaProfile::new(
        ArenaProfileKind::IosTouch,
        [first.clone(), second.clone()],
        [CompoundClaimDescriptor {
            id: CompoundClaimId(100),
            policy: CompoundPolicy::Simultaneous,
            members: vec![ArenaClaimId(1), ArenaClaimId(2)],
        }],
        [ArenaRelationship::Simultaneous {
            first: ArenaClaimId(1),
            second: ArenaClaimId(2),
        }],
    )
    .expect("valid simultaneous compound");
    let snapshot = eligibility(
        source(),
        BackMode::None,
        &[(1, true, false), (2, true, false)],
    );
    let mut arena = GestureArena::new(simultaneous_profile);
    arena
        .begin_stream(
            GestureStreamId(1),
            ArenaInputKind::Touch,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin");
    let resolution = arena
        .activate(
            GestureStreamId(1),
            [ArenaClaimId(2), ArenaClaimId(1)],
            &snapshot,
        )
        .expect("activate");
    assert_eq!(
        resolution.holder,
        Some(LeaseHolder::Compound(CompoundClaimId(100)))
    );
    assert_eq!(
        resolution.active_members,
        vec![ArenaClaimId(1), ArenaClaimId(2)]
    );

    arena
        .begin_stream(
            GestureStreamId(9),
            ArenaInputKind::Touch,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin staggered compound");
    let first_activation = arena
        .activate(GestureStreamId(9), [ArenaClaimId(1)], &snapshot)
        .expect("first child activates");
    assert_eq!(first_activation.active_members, vec![ArenaClaimId(1)]);
    let joined = arena
        .activate(GestureStreamId(9), [ArenaClaimId(2)], &snapshot)
        .expect("second child joins existing compound lease");
    assert_eq!(
        joined.active_members,
        vec![ArenaClaimId(1), ArenaClaimId(2)]
    );

    first.compound = Some(CompoundClaimId(101));
    second.compound = Some(CompoundClaimId(101));
    let exclusive_profile = ArenaProfile::new(
        ArenaProfileKind::IosTouch,
        [first, second],
        [CompoundClaimDescriptor {
            id: CompoundClaimId(101),
            policy: CompoundPolicy::Exclusive,
            members: vec![ArenaClaimId(2), ArenaClaimId(1)],
        }],
        [ArenaRelationship::Exclusive {
            preferred: ArenaClaimId(2),
            fallback: ArenaClaimId(1),
        }],
    )
    .expect("valid exclusive compound");
    let mut arena = GestureArena::new(exclusive_profile);
    arena
        .begin_stream(
            GestureStreamId(2),
            ArenaInputKind::Touch,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin");
    let resolution = arena
        .activate(
            GestureStreamId(2),
            [ArenaClaimId(1), ArenaClaimId(2)],
            &snapshot,
        )
        .expect("activate");
    assert_eq!(
        resolution.holder,
        Some(LeaseHolder::Compound(CompoundClaimId(101)))
    );
    assert_eq!(resolution.active_members, vec![ArenaClaimId(2)]);
}

#[test]
fn tie_break_is_deterministic_across_adversarial_request_order() {
    let profile = ArenaProfile::new(
        ArenaProfileKind::DesktopWeb,
        [
            claim(30, ArenaClaimKind::Gesture, 4, 9, 2),
            claim(20, ArenaClaimKind::Gesture, 4, 9, 1),
            claim(10, ArenaClaimKind::Gesture, 4, 9, 1),
        ],
        [],
        [],
    )
    .expect("valid profile");
    let snapshot = eligibility(
        source(),
        BackMode::None,
        &[(10, true, false), (20, true, false), (30, true, false)],
    );
    for (index, order) in [
        vec![ArenaClaimId(30), ArenaClaimId(20), ArenaClaimId(10)],
        vec![ArenaClaimId(10), ArenaClaimId(30), ArenaClaimId(20)],
        vec![ArenaClaimId(20), ArenaClaimId(10), ArenaClaimId(30)],
    ]
    .into_iter()
    .enumerate()
    {
        let stream = GestureStreamId(index as u64 + 1);
        let mut arena = GestureArena::new(profile.clone());
        arena
            .begin_stream(stream, ArenaInputKind::Pointer, source(), order.clone())
            .expect("begin");
        let resolution = arena.activate(stream, order, &snapshot).expect("activate");
        assert_eq!(
            resolution.holder,
            Some(LeaseHolder::Claim(ArenaClaimId(10)))
        );
    }
}

#[test]
fn back_mode_not_raw_can_go_back_gates_router_and_macos_marks_pager_first() {
    let ios_profile = ArenaProfile::new(
        ArenaProfileKind::IosTouch,
        [
            claim(1, ArenaClaimKind::RouterHistory, 1, 0, 0),
            claim(2, ArenaClaimKind::Scroll, 5, 0, 1),
        ],
        [],
        [],
    )
    .expect("valid profile");
    for (stream_number, mode, scene_lease, expected) in [
        (1, BackMode::PassiveIntent, false, ArenaClaimId(2)),
        // SceneLease failure downgrades interactive to a discrete edge
        // claim; it does not hand the reserved edge to the scroll view.
        (2, BackMode::InteractiveClaim, false, ArenaClaimId(1)),
        (3, BackMode::InteractiveClaim, true, ArenaClaimId(1)),
        (4, BackMode::DiscreteClaim, false, ArenaClaimId(1)),
    ] {
        let mut arena = GestureArena::new(ios_profile.clone());
        let stream = GestureStreamId(stream_number);
        arena
            .begin_stream(
                stream,
                ArenaInputKind::Touch,
                source(),
                [ArenaClaimId(1), ArenaClaimId(2)],
            )
            .expect("begin");
        let snapshot = eligibility(source(), mode, &[(1, true, scene_lease), (2, true, false)]);
        if mode == BackMode::InteractiveClaim && !scene_lease {
            assert_eq!(
                snapshot.effective_back_mode(ArenaClaimId(1)),
                BackMode::DiscreteClaim
            );
        }
        assert_eq!(
            arena
                .activate(stream, [ArenaClaimId(1), ArenaClaimId(2)], &snapshot)
                .expect("activate")
                .holder,
            Some(LeaseHolder::Claim(expected))
        );
    }

    let mac_profile = ArenaProfile::new(
        ArenaProfileKind::MacosTrackpad,
        [
            claim(3, ArenaClaimKind::Pager, 2, 0, 0),
            claim(4, ArenaClaimKind::Scroll, 10, 0, 1),
        ],
        [],
        [],
    )
    .expect("valid profile");
    let mut arena = GestureArena::new(mac_profile);
    arena
        .begin_stream(
            GestureStreamId(5),
            ArenaInputKind::Wheel,
            source(),
            [ArenaClaimId(3), ArenaClaimId(4)],
        )
        .expect("begin");
    let snapshot = eligibility(
        source(),
        BackMode::None,
        &[(3, true, false), (4, true, false)],
    );
    assert_eq!(
        arena
            .activate(
                GestureStreamId(5),
                [ArenaClaimId(4), ArenaClaimId(3)],
                &snapshot,
            )
            .expect("activate")
            .holder,
        Some(LeaseHolder::Claim(ArenaClaimId(3)))
    );
}

#[test]
fn lifecycle_has_three_checkpoints_no_midstream_transfer_and_structural_cancel() {
    let profile = ArenaProfile::new(
        ArenaProfileKind::MacosTrackpad,
        [
            claim(1, ArenaClaimKind::Pager, 3, 0, 0),
            claim(2, ArenaClaimKind::Scroll, 4, 0, 1),
        ],
        [],
        [],
    )
    .expect("valid profile");
    let mut arena = GestureArena::new(profile.clone());
    arena
        .begin_stream(
            GestureStreamId(1),
            ArenaInputKind::Wheel,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin");
    let active_snapshot = eligibility(
        source(),
        BackMode::None,
        &[(1, true, false), (2, true, false)],
    );
    arena
        .activate(
            GestureStreamId(1),
            [ArenaClaimId(1), ArenaClaimId(2)],
            &active_snapshot,
        )
        .expect("activate");
    arena
        .mark_visible_motion(GestureStreamId(1))
        .expect("visible motion");
    assert_eq!(
        arena.stand_down_to_scroll(GestureStreamId(1), ArenaClaimId(1), ArenaClaimId(2)),
        Err(ArenaRuntimeError::TransferAfterAxisLock)
    );
    let ineligible = eligibility(
        InteractionSourceIdentity {
            motion_seq: 99,
            back_policy_revision: 88,
            ..source()
        },
        BackMode::None,
        &[(1, false, false), (2, true, false)],
    );
    assert_eq!(
        arena
            .commit_checkpoint(GestureStreamId(1), &ineligible)
            .expect("checkpoint cancels")
            .reason,
        ArenaDecisionReason::Cancelled
    );
    assert_eq!(
        arena.lease(GestureStreamId(1)).expect("lease").phase,
        LeasePhase::Cancelled
    );

    let mut arena = GestureArena::new(profile.clone());
    arena
        .begin_stream(
            GestureStreamId(2),
            ArenaInputKind::Wheel,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin");
    arena
        .activate(
            GestureStreamId(2),
            [ArenaClaimId(1), ArenaClaimId(2)],
            &active_snapshot,
        )
        .expect("activate");
    let structurally_changed = eligibility(
        InteractionSourceIdentity {
            profile_generation: 500,
            ..source()
        },
        BackMode::None,
        &[(1, true, false), (2, true, false)],
    );
    arena
        .commit_checkpoint(GestureStreamId(2), &structurally_changed)
        .expect("structural cancellation");
    assert_eq!(
        arena
            .lease(GestureStreamId(2))
            .expect("lease")
            .cancel_reason,
        Some(ArenaCancelReason::StructuralInvalidation)
    );

    let mut arena = GestureArena::new(profile);
    arena
        .begin_stream(
            GestureStreamId(3),
            ArenaInputKind::Wheel,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin");
    let changed_before_lock = eligibility(
        InteractionSourceIdentity {
            epoch: 999,
            ..source()
        },
        BackMode::None,
        &[(1, true, false), (2, true, false)],
    );
    assert_eq!(
        arena
            .activate(
                GestureStreamId(3),
                [ArenaClaimId(1), ArenaClaimId(2)],
                &changed_before_lock,
            )
            .expect("pre-lock structural cancellation")
            .reason,
        ArenaDecisionReason::Cancelled
    );
    assert_eq!(
        arena.claim_phase(GestureStreamId(3), ArenaClaimId(1)),
        Some(ClaimPhase::Failed)
    );
}

#[test]
fn commit_dispatch_and_token_ack_hold_busy_lease_through_settle() {
    let profile = ArenaProfile::new(
        ArenaProfileKind::MacosTrackpad,
        [claim(1, ArenaClaimKind::Pager, 1, 0, 0)],
        [],
        [],
    )
    .expect("valid profile");
    let snapshot = eligibility(source(), BackMode::None, &[(1, true, false)]);
    let mut arena = GestureArena::new(profile.clone());
    arena
        .begin_stream(
            GestureStreamId(1),
            ArenaInputKind::Wheel,
            source(),
            [ArenaClaimId(1)],
        )
        .expect("begin");
    arena
        .activate(GestureStreamId(1), [ArenaClaimId(1)], &snapshot)
        .expect("activate");
    arena
        .commit_checkpoint(GestureStreamId(1), &snapshot)
        .expect("commit check");
    arena
        .dispatch_checkpoint(GestureStreamId(1), false)
        .expect("dispatch");
    let token = arena.lease(GestureStreamId(1)).expect("lease").sequence;
    assert_eq!(
        arena.acknowledge_settle(GestureStreamId(1), token + 1),
        Err(ArenaRuntimeError::InvalidLeasePhase)
    );
    arena
        .acknowledge_settle(GestureStreamId(1), token)
        .expect("settle");
    assert_eq!(
        arena.lease(GestureStreamId(1)).expect("lease").phase,
        LeasePhase::Ended
    );

    let mut arena = GestureArena::new(profile);
    arena
        .begin_stream(
            GestureStreamId(2),
            ArenaInputKind::Wheel,
            source(),
            [ArenaClaimId(1)],
        )
        .expect("begin");
    arena
        .activate(GestureStreamId(2), [ArenaClaimId(1)], &snapshot)
        .expect("activate");
    arena
        .commit_checkpoint(GestureStreamId(2), &snapshot)
        .expect("commit check");
    arena
        .dispatch_checkpoint(GestureStreamId(2), true)
        .expect("blocked cancellation");
    assert_eq!(
        arena
            .lease(GestureStreamId(2))
            .expect("lease")
            .cancel_reason,
        Some(ArenaCancelReason::Blocked)
    );
}

#[test]
fn prelock_handoff_is_one_way_and_external_receipts_are_bounded() {
    let profile = ArenaProfile::new(
        ArenaProfileKind::IosTouch,
        [
            claim(1, ArenaClaimKind::Pager, 2, 0, 0),
            claim(2, ArenaClaimKind::Scroll, 3, 0, 1),
        ],
        [],
        [],
    )
    .expect("valid profile");
    let mut arena = GestureArena::with_receipt_capacity(profile, 2);
    arena
        .begin_stream(
            GestureStreamId(1),
            ArenaInputKind::Touch,
            source(),
            [ArenaClaimId(1), ArenaClaimId(2)],
        )
        .expect("begin");
    arena
        .stand_down_to_scroll(GestureStreamId(1), ArenaClaimId(1), ArenaClaimId(2))
        .expect("prelock handoff");
    assert_eq!(
        arena.claim_phase(GestureStreamId(1), ArenaClaimId(1)),
        Some(ClaimPhase::Failed)
    );
    assert_eq!(
        arena.stand_down_to_scroll(GestureStreamId(1), ArenaClaimId(1), ArenaClaimId(2)),
        Err(ArenaRuntimeError::InvalidHandoff)
    );

    for stream in 10..13 {
        arena
            .report_external_owner(
                GestureStreamId(stream),
                ArenaInputKind::PlatformRecognizer,
                source(),
                [ArenaClaimId(1), ArenaClaimId(2)],
                Some(ArenaClaimId(2)),
                77,
            )
            .expect("external receipt");
    }
    assert_eq!(arena.receipts().len(), 2);
    assert!(arena.receipts().iter().all(|receipt| {
        receipt.terminal == Some(ArenaTerminalOutcome::ExternalOwner)
            && receipt.platform_reason_code == Some(77)
    }));
    arena.reset();
    assert!(arena.lease(GestureStreamId(1)).is_none());
}
