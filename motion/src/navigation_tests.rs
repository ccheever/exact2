//! @ref RFC 0100 §4 (the provisional-state lifecycle)

use super::*;

fn disposition() -> BackDisposition {
    BackDisposition {
        structural_revision: 7,
        policy_revision: 11,
        back_target: Some(42),
        back_action: BackActionDisposition::StackPop,
        handler: HandlerDisposition::None,
        data: DataDisposition::Fresh,
        data_usable_until_ms: Some(5_000.0),
        gesture_enabled: true,
        blocker: BlockerDisposition::None,
        reentrant_gesture_active: false,
    }
}

fn presenter() -> PresenterReadiness {
    PresenterReadiness {
        presenter_generation: 3,
        retained_scene_available: true,
        pinned_target: Some(42),
        lease_available: true,
    }
}

fn model() -> InteractiveBackModel {
    InteractiveBackModel::new(
        DecisionIdentity {
            root_instance: 9,
            transition_generation: 1,
        },
        DEFAULT_CONFIRMATION_TIMEOUT_MS,
    )
    .unwrap()
}

#[test]
fn activation_table_covers_interactive_passive_discrete_and_clean_failure() {
    assert_eq!(
        classify_activation(disposition(), Some(presenter()), 100.0).unwrap(),
        ActivationOutcome::Interactive
    );
    let mut blocked = disposition();
    blocked.blocker = BlockerDisposition::Blocked;
    assert_eq!(
        classify_activation(blocked, Some(presenter()), 100.0).unwrap(),
        ActivationOutcome::PassiveBlockedAttempt
    );
    let mut tab = disposition();
    tab.back_action = BackActionDisposition::TabPop;
    assert_eq!(
        classify_activation(tab, Some(presenter()), 100.0).unwrap(),
        ActivationOutcome::Interactive
    );
    let mut root = disposition();
    root.back_target = None;
    assert_eq!(
        classify_activation(root, Some(presenter()), 100.0).unwrap(),
        ActivationOutcome::CleanFailure
    );
    root.handler = HandlerDisposition::Registered;
    assert_eq!(
        classify_activation(root, Some(presenter()), 100.0).unwrap(),
        ActivationOutcome::DiscreteFallback
    );
    let mut expired = disposition();
    expired.data = DataDisposition::Stale;
    expired.data_usable_until_ms = Some(99.0);
    assert_eq!(
        classify_activation(expired, Some(presenter()), 100.0).unwrap(),
        ActivationOutcome::DiscreteFallback
    );
}

#[test]
fn release_before_ack_settles_then_late_allow_and_token_release() {
    let mut model = model();
    model
        .activate(disposition(), Some(presenter()), 0.0)
        .unwrap();
    let release = model
        .release(
            ReleaseSample {
                progress: 0.7,
                velocity_per_second: 120.0,
            },
            DEFAULT_COMMIT_VELOCITY_PER_SECOND,
        )
        .unwrap();
    assert_eq!(release.as_slice()[0], ControllerEffect::BeginCommitSettle);
    assert!(model.settle_visual(100.0).unwrap().as_slice().is_empty());
    assert_eq!(model.phase(), InteractivePhase::SettledPendingConfirmation);
    let confirmed = model
        .confirm(
            disposition(),
            CanonicalMatchOutcome::Authorized { pinned_target: 42 },
        )
        .unwrap();
    assert!(confirmed
        .as_slice()
        .contains(&ControllerEffect::CommitLogicalNavigation));
    let token = model.observe_commit_token().unwrap();
    assert_eq!(model.phase(), InteractivePhase::Settled);
    assert_eq!(
        token.as_slice(),
        &[
            ControllerEffect::ReleaseRetention,
            ControllerEffect::TransitionEnd
        ]
    );
}

#[test]
fn forced_cancel_and_structural_nack_are_durable() {
    for (outcome, expected) in [
        (
            CanonicalMatchOutcome::PolicyDenied,
            DecisionState::ConfirmedCancel,
        ),
        (
            CanonicalMatchOutcome::StructuralDivergence,
            DecisionState::Nacked,
        ),
    ] {
        let mut model = model();
        model
            .activate(disposition(), Some(presenter()), 0.0)
            .unwrap();
        model.confirm(disposition(), outcome).unwrap();
        assert_eq!(model.decision_state().unwrap(), expected);
        model
            .release(
                ReleaseSample {
                    progress: 0.9,
                    velocity_per_second: 900.0,
                },
                DEFAULT_COMMIT_VELOCITY_PER_SECOND,
            )
            .unwrap();
        assert_eq!(model.phase(), InteractivePhase::SettlingCancel);
    }
}

#[test]
fn timeout_expires_pending_but_never_overwrites_allow() {
    let mut pending = model();
    pending
        .activate(disposition(), Some(presenter()), 0.0)
        .unwrap();
    pending
        .release(
            ReleaseSample {
                progress: 0.8,
                velocity_per_second: 0.0,
            },
            DEFAULT_COMMIT_VELOCITY_PER_SECOND,
        )
        .unwrap();
    pending.settle_visual(10.0).unwrap();
    let effects = pending.confirmation_timeout(1_011.0).unwrap();
    assert_eq!(pending.decision_state().unwrap(), DecisionState::Expired);
    assert_eq!(pending.phase(), InteractivePhase::Interrupted);
    assert!(effects
        .as_slice()
        .contains(&ControllerEffect::ReconcileLogicalTruth));

    let mut confirmed = model();
    confirmed
        .activate(disposition(), Some(presenter()), 0.0)
        .unwrap();
    confirmed
        .release(
            ReleaseSample {
                progress: 0.8,
                velocity_per_second: 0.0,
            },
            DEFAULT_COMMIT_VELOCITY_PER_SECOND,
        )
        .unwrap();
    confirmed.settle_visual(10.0).unwrap();
    confirmed
        .confirm(
            disposition(),
            CanonicalMatchOutcome::Authorized { pinned_target: 42 },
        )
        .unwrap();
    let effects = confirmed.confirmation_timeout(1_011.0).unwrap();
    assert_eq!(
        confirmed.decision_state().unwrap(),
        DecisionState::ConfirmedAllow
    );
    assert_eq!(
        effects.as_slice(),
        &[ControllerEffect::DiagnoseRuntimeStall]
    );
}

#[test]
fn reset_tombstones_every_armed_state_and_late_writer_noops() {
    for terminal in [
        None,
        Some(DecisionState::ConfirmedAllow),
        Some(DecisionState::ConfirmedCancel),
        Some(DecisionState::Nacked),
    ] {
        let mut model = model();
        model
            .activate(disposition(), Some(presenter()), 0.0)
            .unwrap();
        if let Some(terminal) = terminal {
            assert!(model.cell.decide(model.identity, terminal).unwrap());
        }
        model.reset().unwrap();
        assert_eq!(model.decision_state().unwrap(), DecisionState::Tombstone);
        assert!(!model
            .cell
            .decide(model.identity, DecisionState::ConfirmedAllow)
            .unwrap());
    }
}

#[test]
fn dispatch_enforcement_is_conjunctive() {
    let all = DispatchEnforcement {
        decision_confirmed_allow: true,
        structural_revision_matches: true,
        policy_revision_matches: true,
        blocker_clear: true,
        access_allowed: true,
        layer_policy_allowed: true,
        target_identity_matches: true,
        handler_clear: true,
        data_deadline_valid: true,
    };
    assert!(all.permits_commit());
    let mutations: [fn(&mut DispatchEnforcement); 9] = [
        |v| v.decision_confirmed_allow = false,
        |v| v.structural_revision_matches = false,
        |v| v.policy_revision_matches = false,
        |v| v.blocker_clear = false,
        |v| v.access_allowed = false,
        |v| v.layer_policy_allowed = false,
        |v| v.target_identity_matches = false,
        |v| v.handler_clear = false,
        |v| v.data_deadline_valid = false,
    ];
    for mutate in mutations {
        let mut candidate = all;
        mutate(&mut candidate);
        assert!(!candidate.permits_commit());
    }
}

#[test]
fn checkpoint_matrix_names_every_field_and_duty() {
    assert_eq!(CHECKPOINT_MATRIX.len(), 11);
    for row in CHECKPOINT_MATRIX {
        assert!(
            row.activation && row.commit_revalidation && row.dispatch_enforcement,
            "missing duty: {:?}",
            row.field
        );
    }
    assert_eq!(
        CHECKPOINT_MATRIX
            .iter()
            .find(|row| row.field == DispositionField::DataUsableUntil)
            .unwrap()
            .classification,
        ChangeClassification::Policy
    );
}

#[test]
fn gate_probe_rejects_all_three_declared_mutations() {
    assert!(gate_probe_passes(&run_gate_probe(GateMutation::None)));
    for mutation in [
        GateMutation::OneFrameDelay,
        GateMutation::MagnitudeOnlyReverseFling,
        GateMutation::RawOneShotLayerWrite,
    ] {
        assert!(
            !gate_probe_passes(&run_gate_probe(mutation)),
            "gate accepted {mutation:?}"
        );
    }
}

#[test]
fn only_one_writer_can_decide_and_a_reset_supersedes_the_winner() {
    // The authorizing writer, the expiry writer, and a reset all race one
    // armed cell. At most one decision may win, and the reset must supersede
    // whichever did.
    for terminal in [
        DecisionState::ConfirmedAllow,
        DecisionState::ConfirmedCancel,
        DecisionState::Nacked,
    ] {
        for _ in 0..256 {
            let identity = DecisionIdentity {
                root_instance: 7,
                transition_generation: 11,
            };
            let cell = Arc::new(DecisionCell::default());
            cell.arm(identity).expect("arm");

            let decided = std::thread::scope(|scope| {
                let authorize = scope.spawn(|| cell.decide(identity, terminal).expect("decide"));
                let expire = scope.spawn(|| {
                    cell.decide(identity, DecisionState::Expired)
                        .expect("expire")
                });
                let authorized = authorize.join().expect("authorizing thread");
                let expired = expire.join().expect("expiring thread");
                usize::from(authorized) + usize::from(expired)
            });
            assert!(decided <= 1, "two writers decided the same transition");

            assert!(cell.tombstone(identity).expect("tombstone"));
            assert_eq!(cell.load().expect("load").1, DecisionState::Tombstone);
            assert!(!cell.decide(identity, terminal).expect("late decide"));
        }
    }
}

#[test]
fn a_decision_word_packs_identity_at_its_declared_boundaries() {
    // rootInstance:24 | transitionGeneration:32 | state:8. The widest legal
    // identity must survive a round trip: if the fields overlapped, the
    // generation would bleed into the root instance here and nowhere else.
    let widest = DecisionIdentity {
        root_instance: DECISION_ROOT_INSTANCE_MAX,
        transition_generation: u32::MAX,
    };
    let packed = pack_decision(widest, DecisionState::Nacked).expect("boundary identity");
    assert_eq!(
        unpack_decision(packed),
        Some((widest, DecisionState::Nacked))
    );

    // One past the 24-bit root instance is refused rather than truncated into
    // some other root's word.
    assert_eq!(
        pack_decision(
            DecisionIdentity {
                root_instance: DECISION_ROOT_INSTANCE_MAX + 1,
                transition_generation: 1,
            },
            DecisionState::Pending,
        ),
        Err(InteractiveNavigationError::DecisionIdentityOverflow)
    );

    // Neither field has a meaningful zero.
    for zeroed in [
        DecisionIdentity {
            root_instance: 0,
            transition_generation: 1,
        },
        DecisionIdentity {
            root_instance: 1,
            transition_generation: 0,
        },
    ] {
        assert_eq!(
            pack_decision(zeroed, DecisionState::Pending),
            Err(InteractiveNavigationError::ZeroDecisionIdentity)
        );
    }
}

#[test]
fn only_a_later_generation_of_the_same_root_may_rearm_a_finished_cell() {
    let cell = DecisionCell::default();
    let identity = DecisionIdentity {
        root_instance: 11,
        transition_generation: 4,
    };
    cell.arm(identity).expect("arm");
    assert!(cell
        .decide(identity, DecisionState::ConfirmedAllow)
        .expect("valid terminal"));
    assert!(!cell
        .decide(identity, DecisionState::Expired)
        .expect("late terminal is a no-op"));
    assert_eq!(
        cell.load().expect("valid word").1,
        DecisionState::ConfirmedAllow
    );

    // A different root cannot claim a cell that another root finished.
    assert_eq!(
        cell.arm(DecisionIdentity {
            root_instance: 12,
            transition_generation: 1,
        }),
        Err(InteractiveNavigationError::InvalidDecisionTransition)
    );
    // Nor may the same generation arm itself twice.
    assert_eq!(
        cell.arm(identity),
        Err(InteractiveNavigationError::InvalidDecisionTransition)
    );

    // A strictly later generation of the same root may.
    let successor = DecisionIdentity {
        transition_generation: 5,
        ..identity
    };
    cell.arm(successor).expect("successor generation");
    assert_eq!(
        cell.load().expect("valid word"),
        (successor, DecisionState::Pending)
    );

    // The same holds across a tombstone.
    assert!(cell.tombstone(successor).expect("tombstone"));
    assert_eq!(
        cell.arm(successor),
        Err(InteractiveNavigationError::InvalidDecisionTransition)
    );
    cell.arm(DecisionIdentity {
        transition_generation: 6,
        ..identity
    })
    .expect("generation after a tombstone");
}
