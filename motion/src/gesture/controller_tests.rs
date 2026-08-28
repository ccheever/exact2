//! @ref LLP 0099#gesture-composition

use super::*;
use crate::plan::{MotionPlan, PlanNode, PlanNodeKind};

const ROOT: u64 = 8;
const NODE: u32 = 3;
const INSTANCE: u32 = 9;
const EPOCH: u32 = 7;
const PRESENTER: u32 = 13;

fn pan(id: u64, minimum_distance: f64) -> PlanNode {
    PlanNode {
        id,
        generation: 1,
        node_id: NODE,
        kind: PlanNodeKind::Recognizer {
            descriptor: RecognizerDescriptor::Pan(PanDescriptor {
                common: RecognizerCommon {
                    id: GestureId(id),
                    generation: 1,
                    min_pointers: 1,
                    max_pointers: 1,
                },
                axis: GestureAxis::Horizontal,
                minimum_distance,
                axis_lock_ratio: 1.25,
                fail_cross_axis: false,
                active_offset_x: None,
                active_offset_y: None,
            }),
            claim_direction: None,
        },
    }
}

fn tap(id: u64) -> PlanNode {
    PlanNode {
        id,
        generation: 1,
        node_id: NODE,
        kind: PlanNodeKind::Recognizer {
            descriptor: RecognizerDescriptor::Tap(TapDescriptor {
                common: RecognizerCommon {
                    id: GestureId(id),
                    generation: 1,
                    min_pointers: 1,
                    max_pointers: 1,
                },
                tap_count: 1,
                maximum_duration_ms: 300.0,
                maximum_distance: 10.0,
                maximum_inter_tap_ms: 300.0,
            }),
            claim_direction: None,
        },
    }
}

fn composition(id: u64, kind: CompositionKind, children: Vec<u64>) -> PlanNode {
    PlanNode {
        id,
        generation: 1,
        node_id: NODE,
        kind: PlanNodeKind::Composition { kind, children },
    }
}

fn environment() -> InteractionEnvironmentSnapshot {
    InteractionEnvironmentSnapshot {
        root_id: ROOT,
        presenter_generation: PRESENTER,
        focused_root_id: Some(ROOT),
        window_id: Some(1),
        effective_layout_direction: LayoutDirection::LeftToRight,
        display_mode: InteractionDisplayMode::Touch,
        host_chrome_present: false,
    }
}

fn topology(motion_sequence: u64) -> InteractionTopologyContext {
    InteractionTopologyContext {
        root_id: ROOT,
        root_instance: u64::from(INSTANCE),
        epoch: EPOCH,
        motion_seq: motion_sequence,
        modal_barrier_id: None,
        modal_generation: 0,
        declared_layout_direction: LayoutDirection::LeftToRight,
        back_mode: BackMode::None,
        back_structural_revision: 0,
        back_policy_revision: 0,
    }
}

/// A controller with `plan` installed and every claim published consumable.
fn controller(plan: &MotionPlan) -> GestureGraphController {
    let mut controller =
        GestureGraphController::new(ROOT, INSTANCE, EPOCH, 1, ArenaProfileKind::IosTouch)
            .expect("valid identity");
    controller
        .publish_environment(environment())
        .expect("environment");
    controller
        .replace(INSTANCE, EPOCH, 1, plan, 0.0)
        .expect("install plan");
    let eligibility = controller
        .claims()
        .map(|claim| ClaimEligibilityRecord {
            claim_id: claim.0,
            presenter_generation: PRESENTER,
            can_consume: true,
            scene_lease_acquired: false,
        })
        .collect::<Vec<_>>();
    controller
        .publish_interaction_state(topology(1), &eligibility, &[])
        .expect("interaction state");
    controller.drain();
    controller
}

fn frame(
    stream: u64,
    phase: PointerPhase,
    timestamp_ms: f64,
    x: f64,
    y: f64,
    delta: (f64, f64),
) -> GestureFrame {
    GestureFrame {
        root_instance: INSTANCE,
        epoch: EPOCH,
        motion_sequence: 1,
        node_id: NODE,
        stream_id: stream,
        timestamp_ms,
        phase,
        input_kind: ArenaInputKind::Touch,
        contacts: vec![PointerContact {
            id: PointerId(1),
            position: MotionPoint { x, y },
            absolute_position: MotionPoint { x, y },
            delta: MotionPoint {
                x: delta.0,
                y: delta.1,
            },
            pressure: 0.0,
        }],
    }
}

fn drag(controller: &mut GestureGraphController, stream: u64) -> Vec<GestureOutput> {
    controller
        .feed(&frame(
            stream,
            PointerPhase::Down,
            0.0,
            0.0,
            0.0,
            (0.0, 0.0),
        ))
        .expect("down");
    controller
        .feed(&frame(
            stream,
            PointerPhase::Move,
            16.0,
            40.0,
            0.0,
            (40.0, 0.0),
        ))
        .expect("move");
    controller
        .feed(&frame(
            stream,
            PointerPhase::Move,
            32.0,
            80.0,
            0.0,
            (40.0, 0.0),
        ))
        .expect("move");
    controller.drain()
}

#[test]
fn a_plan_becomes_one_arena_claim_per_root() {
    let plan = MotionPlan::new(vec![
        pan(1, 10.0),
        tap(2),
        composition(3, CompositionKind::Exclusive, vec![1, 2]),
        pan(4, 10.0),
    ]);
    assert_eq!(plan.validate(), Ok(()));
    let graph = GestureGraph::build(&plan, ArenaProfileKind::IosTouch).expect("build");
    // Two roots: the composition, and the pan that nothing composes.
    assert_eq!(
        graph.claims().map(|claim| claim.0).collect::<Vec<_>>(),
        vec![3, 4]
    );
}

#[test]
fn the_builder_refuses_graphs_the_runtime_could_not_run() {
    let cyclic = MotionPlan::new(vec![
        pan(1, 10.0),
        composition(2, CompositionKind::Race, vec![3, 1]),
        composition(3, CompositionKind::Race, vec![2, 1]),
    ]);
    assert_eq!(
        GestureGraph::build(&cyclic, ArenaProfileKind::IosTouch).err(),
        Some(ControllerError::InvalidGraph)
    );

    // A composition is a tree: two parents cannot share one recognizer.
    let shared = MotionPlan::new(vec![
        pan(1, 10.0),
        tap(2),
        tap(5),
        composition(3, CompositionKind::Race, vec![1, 2]),
        composition(4, CompositionKind::Race, vec![1, 5]),
    ]);
    assert_eq!(
        GestureGraph::build(&shared, ArenaProfileKind::IosTouch).err(),
        Some(ControllerError::InvalidGraph)
    );

    // Every descendant of a root sits on the root's own view node.
    let mut straddling = pan(2, 10.0);
    straddling.node_id = NODE + 1;
    let split = MotionPlan::new(vec![
        pan(1, 10.0),
        straddling,
        composition(3, CompositionKind::Race, vec![1, 2]),
    ]);
    assert_eq!(
        GestureGraph::build(&split, ArenaProfileKind::IosTouch).err(),
        Some(ControllerError::InvalidGraph)
    );

    // A dangling child, and a composition with fewer than two children.
    let dangling = MotionPlan::new(vec![
        pan(1, 10.0),
        composition(2, CompositionKind::Race, vec![1, 99]),
    ]);
    assert_eq!(
        GestureGraph::build(&dangling, ArenaProfileKind::IosTouch).err(),
        Some(ControllerError::InvalidGraph)
    );
    let lonely = MotionPlan::new(vec![
        pan(1, 10.0),
        composition(2, CompositionKind::Race, vec![1]),
    ]);
    assert_eq!(
        GestureGraph::build(&lonely, ArenaProfileKind::IosTouch).err(),
        Some(ControllerError::InvalidGraph)
    );
}

#[test]
fn a_frame_from_a_superseded_plan_is_refused_not_applied() {
    let plan = MotionPlan::new(vec![pan(1, 10.0)]);
    let mut controller = controller(&plan);
    let mut stale = frame(1, PointerPhase::Down, 0.0, 0.0, 0.0, (0.0, 0.0));
    stale.motion_sequence = 99;
    assert_eq!(controller.feed(&stale), Err(ControllerError::StaleFrame));

    // Replacing forward is fine; replacing backward is not.
    assert_eq!(
        controller.replace(INSTANCE, EPOCH, 1, &plan, 0.0),
        Err(ControllerError::StaleFrame)
    );
}

#[test]
fn a_pan_wins_its_stream_and_emits_a_lifecycle_that_needs_resolution() {
    let plan = MotionPlan::new(vec![pan(1, 10.0)]);
    let mut controller = controller(&plan);
    let outputs = drag(&mut controller, 1);

    let lifecycle: Vec<_> = outputs
        .iter()
        .filter(|output| output.effects.lifecycle)
        .collect();
    assert_eq!(lifecycle[0].phase, GestureOutputPhase::Began);
    assert_eq!(lifecycle[0].gesture_kind, Some(RecognizerKind::Pan));
    assert_eq!(lifecycle[0].root_gesture_id, 1);
    assert!(lifecycle[0].composition_instance_id != 0);
    assert!(lifecycle
        .iter()
        .any(|output| output.phase == GestureOutputPhase::Changed
            && matches!(output.payload, GesturePayload::Pan { .. })));

    // Lifting the finger ends recognition, but not the outcome.
    controller
        .feed(&frame(1, PointerPhase::Up, 48.0, 80.0, 0.0, (0.0, 0.0)))
        .expect("up");
    let ended = controller.drain();
    let needs_resolution = ended
        .iter()
        .find(|output| output.effects.needs_resolution)
        .expect("recognition ended pending resolution");
    assert_eq!(needs_resolution.phase, GestureOutputPhase::Ended);
    assert!(!needs_resolution.effects.stream_terminal);

    controller
        .resolve_terminal(1, true, false, 48.0)
        .expect("commit");
    let resolved = controller.drain();
    let terminal = resolved
        .iter()
        .find(|output| output.effects.stream_terminal)
        .expect("stream terminal");
    assert!(terminal.committed);
    assert_eq!(
        terminal.receipt,
        Some(GestureReceipt::Arena(ArenaDecisionReason::DispatchAllowed))
    );
    assert_eq!(controller.active_stream_count(), 0);
}

#[test]
fn a_cancelled_resolution_terminates_the_stream_without_committing() {
    let plan = MotionPlan::new(vec![pan(1, 10.0)]);
    let mut controller = controller(&plan);
    drag(&mut controller, 1);
    controller
        .feed(&frame(1, PointerPhase::Up, 48.0, 80.0, 0.0, (0.0, 0.0)))
        .expect("up");
    controller.drain();

    controller
        .resolve_terminal(1, false, false, 48.0)
        .expect("cancel");
    let outputs = controller.drain();
    let terminal = outputs
        .iter()
        .find(|output| output.effects.stream_terminal)
        .expect("stream terminal");
    assert!(!terminal.committed);
    assert_eq!(terminal.phase, GestureOutputPhase::Cancelled);
    assert_eq!(
        terminal.terminal_reason,
        Some(GestureTerminalReason::AuthorCancelled)
    );
    assert_eq!(controller.active_stream_count(), 0);
}

#[test]
fn an_exclusive_composition_publishes_its_winner_and_cancels_the_rest() {
    // The tap is declared first, so the pan can only win once the tap fails —
    // which a drag makes it do.
    let plan = MotionPlan::new(vec![
        tap(1),
        pan(2, 10.0),
        composition(3, CompositionKind::Exclusive, vec![1, 2]),
    ]);
    let mut controller = controller(&plan);
    let outputs = drag(&mut controller, 1);

    let receipts: Vec<_> = outputs
        .iter()
        .filter_map(|output| match output.receipt {
            Some(GestureReceipt::Composition(kind)) => Some((kind, output.decided_child_id)),
            _ => None,
        })
        .collect();
    assert!(receipts.contains(&(CompositionDecisionKind::ExclusiveWinner, 2)));
    assert!(receipts.contains(&(CompositionDecisionKind::ExclusiveLoser, 1)));

    // Only the winner's events are author-visible.
    assert!(outputs
        .iter()
        .filter(|output| output.effects.lifecycle)
        .all(|output| output.gesture_id == 2));
}

#[test]
fn a_race_publishes_one_winner_and_the_losers_never_emit() {
    let plan = MotionPlan::new(vec![
        pan(1, 10.0),
        pan(2, 400.0),
        composition(3, CompositionKind::Race, vec![1, 2]),
    ]);
    let mut controller = controller(&plan);
    let outputs = drag(&mut controller, 1);

    let winners: Vec<_> = outputs
        .iter()
        .filter(|output| {
            output.receipt
                == Some(GestureReceipt::Composition(
                    CompositionDecisionKind::RaceWinner,
                ))
        })
        .map(|output| output.decided_child_id)
        .collect();
    assert_eq!(winners, vec![1]);
    assert!(outputs.iter().any(|output| output.receipt
        == Some(GestureReceipt::Composition(
            CompositionDecisionKind::RaceLoser
        ))));
    assert!(outputs
        .iter()
        .filter(|output| output.effects.lifecycle)
        .all(|output| output.gesture_id == 1));
}

#[test]
fn a_simultaneous_composition_leases_once_and_every_child_emits() {
    let plan = MotionPlan::new(vec![
        pan(1, 10.0),
        pan(2, 10.0),
        composition(3, CompositionKind::Simultaneous, vec![1, 2]),
    ]);
    let mut controller = controller(&plan);
    let outputs = drag(&mut controller, 1);

    let leases = outputs
        .iter()
        .filter(|output| {
            output.receipt
                == Some(GestureReceipt::Composition(
                    CompositionDecisionKind::SimultaneousLease,
                ))
        })
        .count();
    assert_eq!(leases, 1, "one lease covers every child");
    let emitting: BTreeSet<_> = outputs
        .iter()
        .filter(|output| output.effects.lifecycle)
        .map(|output| output.gesture_id)
        .collect();
    assert_eq!(emitting, BTreeSet::from([1, 2]));
}

#[test]
fn a_sequence_survives_the_gap_between_pointer_streams() {
    let plan = MotionPlan::new(vec![
        tap(1),
        tap(2),
        composition(3, CompositionKind::Sequence, vec![1, 2]),
    ]);
    let mut controller = controller(&plan);

    // First tap: down and up on one stream.
    controller
        .feed(&frame(1, PointerPhase::Down, 0.0, 0.0, 0.0, (0.0, 0.0)))
        .expect("down");
    controller
        .feed(&frame(1, PointerPhase::Up, 40.0, 0.0, 0.0, (0.0, 0.0)))
        .expect("up");
    let first = controller.drain();
    assert!(first.iter().any(|output| output.receipt
        == Some(GestureReceipt::Composition(
            CompositionDecisionKind::SequenceAdvance
        ))));
    assert_eq!(controller.active_stream_count(), 0);
    assert_eq!(
        controller.continuation_count(),
        1,
        "the composition waits for the next stream"
    );

    // Second tap on a new stream completes the sequence.
    controller
        .feed(&frame(2, PointerPhase::Down, 80.0, 0.0, 0.0, (0.0, 0.0)))
        .expect("down");
    controller
        .feed(&frame(2, PointerPhase::Up, 120.0, 0.0, 0.0, (0.0, 0.0)))
        .expect("up");
    let second = controller.drain();
    assert!(second.iter().any(|output| output.receipt
        == Some(GestureReceipt::Composition(
            CompositionDecisionKind::SequenceComplete
        ))));
    assert_eq!(controller.continuation_count(), 0);
}

#[test]
fn detaching_the_topology_drops_streams_and_waiting_continuations() {
    let plan = MotionPlan::new(vec![
        tap(1),
        tap(2),
        composition(3, CompositionKind::Sequence, vec![1, 2]),
    ]);
    let mut controller = controller(&plan);
    controller
        .feed(&frame(1, PointerPhase::Down, 0.0, 0.0, 0.0, (0.0, 0.0)))
        .expect("down");
    controller
        .feed(&frame(1, PointerPhase::Up, 40.0, 0.0, 0.0, (0.0, 0.0)))
        .expect("up");
    controller.drain();
    assert_eq!(controller.continuation_count(), 1);

    controller.detach_topology(80.0).expect("detach");
    assert_eq!(controller.continuation_count(), 0);
    assert_eq!(controller.active_stream_count(), 0);
}

#[test]
fn an_external_owner_cancels_a_live_stream_and_retires_an_unseen_one() {
    let plan = MotionPlan::new(vec![pan(1, 10.0)]);
    let mut controller = controller(&plan);
    drag(&mut controller, 1);
    controller
        .report_external_owner(NODE, 1, ArenaInputKind::Touch, 42, 48.0)
        .expect("external owner");
    let outputs = controller.drain();
    assert!(outputs.iter().any(|output| output.terminal_reason
        == Some(GestureTerminalReason::ExternalOwner)
        && output.effects.stream_terminal));
    assert_eq!(controller.active_stream_count(), 0);

    // The platform can also win before the first sample ever arrives.
    controller
        .report_external_owner(NODE, 2, ArenaInputKind::Touch, 42, 64.0)
        .expect("external owner before admission");
    let unseen = controller.drain();
    assert!(unseen
        .iter()
        .any(|output| output.effects.stream_retired && !output.effects.terminal));
}

#[test]
fn a_claim_the_presenter_omits_cannot_win() {
    let plan = MotionPlan::new(vec![pan(1, 10.0)]);
    let mut controller = controller(&plan);
    // Republish with the claim explicitly unable to consume.
    controller
        .publish_interaction_state(
            topology(1),
            &[ClaimEligibilityRecord {
                claim_id: 1,
                presenter_generation: PRESENTER,
                can_consume: false,
                scene_lease_acquired: false,
            }],
            &[],
        )
        .expect("interaction state");
    let outputs = drag(&mut controller, 1);
    assert!(
        outputs.iter().all(|output| !output.effects.lifecycle),
        "an ineligible claim never emits an author-visible event"
    );

    // Omitting a mounted claim entirely is refused rather than defaulted.
    assert_eq!(
        controller.publish_interaction_state(topology(1), &[], &[]),
        Err(ControllerError::InteractionState)
    );
}

#[test]
fn a_host_that_never_drains_is_refused_rather_than_grown_without_bound() {
    let plan = MotionPlan::new(vec![pan(1, 10.0)]);
    let mut controller = controller(&plan);
    controller
        .feed(&frame(1, PointerPhase::Down, 0.0, 0.0, 0.0, (0.0, 0.0)))
        .expect("down");

    // Keep the stream alive and never drain. Every accepted move retains at
    // least one output, so the buffer must reach the cap and the next call
    // must be refused instead of allocating further.
    let mut timestamp = 0.0;
    let mut position = 0.0;
    let refusal = loop {
        timestamp += 16.0;
        position += 40.0;
        let outcome = controller.feed(&frame(
            1,
            PointerPhase::Move,
            timestamp,
            position,
            0.0,
            (40.0, 0.0),
        ));
        if let Err(error) = outcome {
            break error;
        }
        assert!(
            controller.outputs().len() <= MAX_RETAINED_OUTPUTS,
            "retained outputs passed the cap without a refusal"
        );
        assert!(
            timestamp < 4.0e6,
            "the buffer never filled: outputs are not accumulating"
        );
    };
    assert_eq!(refusal, ControllerError::OutputBackpressure);

    // The refusal is not terminal: draining restores headroom, and the same
    // call then succeeds.
    let drained = controller.drain();
    assert!(!drained.is_empty());
    assert!(controller.outputs().is_empty());
    controller
        .feed(&frame(
            1,
            PointerPhase::Move,
            timestamp + 16.0,
            position + 40.0,
            0.0,
            (40.0, 0.0),
        ))
        .expect("headroom restored by draining");
}

#[test]
fn a_graph_whose_single_feed_cannot_fit_the_retained_inventory_is_refused() {
    // One node carrying enough leaves that the worst-case single feed exceeds
    // MAX_RETAINED_OUTPUTS is refused at build time, not at the first frame:
    // a fully drained controller must always have room for one feed.
    let leaves = MAX_RETAINED_OUTPUTS / 4 + 2;
    let plan = MotionPlan::new(
        (1..=leaves as u64)
            .map(|id| pan(id, 10.0))
            .collect::<Vec<_>>(),
    );
    assert_eq!(plan.validate(), Ok(()));
    assert_eq!(
        GestureGraph::build(&plan, ArenaProfileKind::IosTouch).err(),
        Some(ControllerError::InvalidGraph)
    );
}
