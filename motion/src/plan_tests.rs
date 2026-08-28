//! @ref RFC 0492 (animation is compiled to plan data)

use super::*;
use crate::driver::{SpringConfig, TimingConfig};
use crate::gesture::{GestureAxis, GestureId, PanDescriptor, RecognizerCommon};

fn shared_value(id: u64) -> PlanNode {
    PlanNode {
        id,
        generation: 1,
        node_id: 1,
        kind: PlanNodeKind::SharedValue { initial: 0.0 },
    }
}

fn binding(id: u64, value: u64, sink: PropertySink) -> PlanNode {
    PlanNode {
        id,
        generation: 1,
        node_id: 1,
        kind: PlanNodeKind::PropertyBinding { value, sink },
    }
}

fn driver(id: u64, value: u64) -> PlanNode {
    PlanNode {
        id,
        generation: 1,
        node_id: 1,
        kind: PlanNodeKind::Driver {
            value,
            spec: AnimationDriverSpec::Spring {
                target: 1.0,
                config: SpringConfig::default(),
            },
            reduced_motion: ReducedMotionAction::Immediate,
        },
    }
}

fn recognizer(id: u64) -> PlanNode {
    PlanNode {
        id,
        generation: 1,
        node_id: 1,
        kind: PlanNodeKind::Recognizer {
            descriptor: RecognizerDescriptor::Pan(PanDescriptor {
                common: RecognizerCommon {
                    id: GestureId(id),
                    generation: 1,
                    min_pointers: 1,
                    max_pointers: 1,
                },
                axis: GestureAxis::Horizontal,
                minimum_distance: 10.0,
                axis_lock_ratio: 1.25,
                fail_cross_axis: false,
                active_offset_x: None,
                active_offset_y: None,
            }),
            claim_direction: None,
        },
    }
}

#[test]
fn a_well_formed_plan_validates() {
    let plan = MotionPlan::new(vec![
        shared_value(1),
        binding(2, 1, PropertySink::TranslateX),
        driver(3, 1),
        recognizer(4),
    ]);
    assert_eq!(plan.validate(), Ok(()));
    assert_eq!(plan.gesture_nodes().count(), 1);
    assert_eq!(plan.node(3).map(|node| node.id), Some(3));
}

#[test]
fn identity_must_be_nonzero_and_unique() {
    let mut zero = shared_value(1);
    zero.id = 0;
    assert_eq!(
        MotionPlan::new(vec![zero]).validate(),
        Err(PlanError::ZeroIdentity)
    );

    let mut ungenerated = shared_value(1);
    ungenerated.generation = 0;
    assert_eq!(
        MotionPlan::new(vec![ungenerated]).validate(),
        Err(PlanError::ZeroIdentity)
    );

    assert_eq!(
        MotionPlan::new(vec![shared_value(1), shared_value(1)]).validate(),
        Err(PlanError::DuplicateId)
    );
}

#[test]
fn edges_must_resolve_to_a_node_of_a_kind_that_can_satisfy_them() {
    assert_eq!(
        MotionPlan::new(vec![binding(2, 9, PropertySink::Opacity)]).validate(),
        Err(PlanError::DanglingDependency)
    );
    assert_eq!(
        MotionPlan::new(vec![recognizer(1), binding(2, 1, PropertySink::Opacity)]).validate(),
        Err(PlanError::WrongDependencyKind)
    );
}

#[test]
fn a_dependency_cycle_is_refused() {
    let derived = |id: u64, inputs: Vec<u64>| PlanNode {
        id,
        generation: 1,
        node_id: 1,
        kind: PlanNodeKind::DerivedValue { inputs },
    };
    assert_eq!(
        MotionPlan::new(vec![derived(1, vec![2]), derived(2, vec![1])]).validate(),
        Err(PlanError::DependencyCycle)
    );
    // The same shape without the back edge is fine, and a diamond is not a
    // cycle even though it reaches one node twice.
    assert_eq!(
        MotionPlan::new(vec![
            shared_value(1),
            derived(2, vec![1]),
            derived(3, vec![1]),
            derived(4, vec![2, 3]),
        ])
        .validate(),
        Ok(())
    );
}

#[test]
fn two_bindings_cannot_drive_one_property_of_one_node() {
    assert_eq!(
        MotionPlan::new(vec![
            shared_value(1),
            shared_value(2),
            binding(3, 1, PropertySink::Opacity),
            binding(4, 2, PropertySink::Opacity),
        ])
        .validate(),
        Err(PlanError::DuplicateSink)
    );
    // The same value on two different properties is allowed.
    assert_eq!(
        MotionPlan::new(vec![
            shared_value(1),
            binding(3, 1, PropertySink::TranslateX),
            binding(4, 1, PropertySink::TranslateY),
        ])
        .validate(),
        Ok(())
    );
}

#[test]
fn non_finite_values_never_reach_the_evaluator() {
    let mut node = shared_value(1);
    node.kind = PlanNodeKind::SharedValue { initial: f32::NAN };
    assert_eq!(
        MotionPlan::new(vec![node]).validate(),
        Err(PlanError::NonFiniteValue)
    );
}

#[test]
fn commands_validate_identity_and_payload() {
    let target = CommandTarget {
        value: 1,
        generation: 1,
        epoch: 1,
    };
    let write = |value: f64| ValueCommand {
        sequence: 1,
        target,
        kind: ValueCommandKind::Write { value },
    };
    assert_eq!(write(1.0).validate(), Ok(()));
    assert_eq!(
        write(f64::INFINITY).validate(),
        Err(PlanError::NonFiniteValue)
    );

    let unsequenced = ValueCommand {
        sequence: 0,
        target,
        kind: ValueCommandKind::CancelDriver,
    };
    assert_eq!(unsequenced.validate(), Err(PlanError::ZeroIdentity));

    let stale = ValueCommand {
        sequence: 1,
        target: CommandTarget {
            generation: 0,
            ..target
        },
        kind: ValueCommandKind::StartDriver {
            spec: AnimationDriverSpec::Timing {
                target: 1.0,
                config: TimingConfig {
                    duration_seconds: 0.2,
                    easing: crate::driver::MotionEasing::Linear,
                },
            },
            reduced_motion: ReducedMotionAction::Unchanged,
            inherit_velocity: true,
        },
    };
    assert_eq!(stale.validate(), Err(PlanError::ZeroIdentity));
}
