//! Plan data: the typed description of a motion graph that a compiler emits
//! and this crate evaluates.
//!
//! @ref RFC 0492 (animation is compiled to plan data)
//! @ref LLP 0099 (motion)
//!
//! Nothing here executes. A [`MotionPlan`] is a flat list of identified nodes —
//! values, bindings, drivers, recognizers, compositions — plus the edges
//! between them, all plain Rust with no bytes, no schema versions, and no
//! interpreter. That is the point: the authored program is compiled ahead of
//! time into records the evaluator can walk without running any of the
//! author's code on the frame path.
//!
//! Commands ([`ValueCommand`]) are the imperative half: the discrete "start
//! this driver", "write this value" events that arrive between frames.
//! Outcomes ([`OutcomeRecord`]) are what the evaluator reports back.

use crate::driver::{AnimationDriverSpec, DriverTerminal};
use crate::gesture::{ArenaDirection, CompositionKind, RecognizerDescriptor};
use std::collections::{BTreeMap, BTreeSet};

/// The property a bound value drives.
///
/// v1 has exactly two sinks. Both are properties a compositor can animate
/// without touching layout, which is what lets one evaluator drive every
/// surface at the same cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PropertySink {
    /// Horizontal translation, in points.
    TranslateX,
    /// Vertical translation, in points.
    TranslateY,
    /// Uniform scale, where one is the natural size.
    Scale,
    /// Rotation about the node's center, in radians.
    Rotate,
    /// Opacity, from zero to one.
    Opacity,
}

/// What a driver does when the viewer has asked for reduced motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReducedMotionAction {
    /// Run the animation as authored.
    #[default]
    Unchanged,
    /// Do not animate and do not change the value.
    Suppress,
    /// Jump straight to the target, emitting one terminal.
    Immediate,
}

/// One node of a compiled motion graph.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanNode {
    /// Stable identity within the plan. Never zero.
    pub id: u64,
    /// Bumped whenever the compiler re-emits this node, so a command or event
    /// that names an older revision can be refused instead of misapplied.
    pub generation: u32,
    /// The view node this record is mounted on. Never zero.
    pub node_id: u32,
    /// What the node is.
    pub kind: PlanNodeKind,
}

/// The kinds of node a plan may contain.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanNodeKind {
    /// A single animatable scalar in the value plane.
    SharedValue {
        /// Value the slot holds before anything writes it.
        initial: f32,
    },
    /// A scalar computed from other values rather than written directly.
    DerivedValue {
        /// Plan ids of the values this one reads, in argument order.
        inputs: Vec<u64>,
    },
    /// A value routed to a property of its node.
    PropertyBinding {
        /// Plan id of the value or derived value that feeds the sink.
        value: u64,
        /// The property it drives.
        sink: PropertySink,
    },
    /// An animation authored against a value.
    Driver {
        /// Plan id of the value the driver owns while it runs.
        value: u64,
        /// The closed-form animation to run.
        spec: AnimationDriverSpec,
        /// What to do instead when reduced motion is in effect.
        reduced_motion: ReducedMotionAction,
    },
    /// One gesture recognizer.
    Recognizer {
        /// The authored recognizer.
        descriptor: RecognizerDescriptor,
        /// Direction this recognizer claims, when it claims only one. A
        /// direction-agnostic recognizer competes with a scroll presenter that
        /// has capacity on either side.
        claim_direction: Option<ArenaDirection>,
    },
    /// A composition of recognizers, or of other compositions.
    Composition {
        /// How the children relate: exclusive, race, simultaneous, sequence.
        kind: CompositionKind,
        /// Plan ids of the children, in authored order.
        children: Vec<u64>,
    },
}

impl PlanNodeKind {
    /// Plan ids this node reads, in authored order.
    pub fn dependencies(&self) -> Vec<u64> {
        match self {
            Self::SharedValue { .. } | Self::Recognizer { .. } => Vec::new(),
            Self::DerivedValue { inputs } => inputs.clone(),
            Self::PropertyBinding { value, .. } | Self::Driver { value, .. } => vec![*value],
            Self::Composition { children, .. } => children.clone(),
        }
    }

    /// Whether this node produces a value other nodes may read.
    pub fn is_value(&self) -> bool {
        matches!(self, Self::SharedValue { .. } | Self::DerivedValue { .. })
    }

    /// Whether this node belongs to the gesture graph.
    pub fn is_gesture(&self) -> bool {
        matches!(self, Self::Recognizer { .. } | Self::Composition { .. })
    }
}

/// Why a plan was refused.
///
/// A plan is validated once, at the boundary. After that the evaluator may
/// assume its shape, which is why these checks are exhaustive rather than
/// convenient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanError {
    /// A node's identity, generation, or view node was zero.
    ZeroIdentity,
    /// Two nodes claim the same plan id.
    DuplicateId,
    /// A node depends on a plan id that is not in the plan.
    DanglingDependency,
    /// A node depends on a node of a kind it cannot read.
    WrongDependencyKind,
    /// A dependency path returns to where it started.
    DependencyCycle,
    /// A shared value's initial value was infinite or NaN.
    NonFiniteValue,
    /// Two bindings drive the same property of the same node.
    DuplicateSink,
}

/// A compiled motion graph.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionPlan {
    /// Every node in the plan, in any order.
    pub nodes: Vec<PlanNode>,
}

impl MotionPlan {
    /// Build a plan from its nodes.
    pub fn new(nodes: Vec<PlanNode>) -> Self {
        Self { nodes }
    }

    /// Look up one node.
    pub fn node(&self, id: u64) -> Option<&PlanNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// The recognizer and composition nodes, in plan order.
    pub fn gesture_nodes(&self) -> impl Iterator<Item = &PlanNode> {
        self.nodes.iter().filter(|node| node.kind.is_gesture())
    }

    /// Check every structural rule the evaluator relies on.
    ///
    /// Identity is nonzero and unique, every dependency resolves to a node of a
    /// kind that can satisfy it, the dependency graph is acyclic, and no two
    /// bindings fight over one property.
    pub fn validate(&self) -> Result<(), PlanError> {
        let mut by_id = BTreeMap::new();
        for node in &self.nodes {
            if node.id == 0 || node.generation == 0 || node.node_id == 0 {
                return Err(PlanError::ZeroIdentity);
            }
            if by_id.insert(node.id, node).is_some() {
                return Err(PlanError::DuplicateId);
            }
            if let PlanNodeKind::SharedValue { initial } = node.kind {
                if !initial.is_finite() {
                    return Err(PlanError::NonFiniteValue);
                }
            }
        }

        let mut sinks = BTreeSet::new();
        for node in &self.nodes {
            for dependency in node.kind.dependencies() {
                let target = by_id
                    .get(&dependency)
                    .ok_or(PlanError::DanglingDependency)?;
                let compatible = match &node.kind {
                    PlanNodeKind::DerivedValue { .. }
                    | PlanNodeKind::PropertyBinding { .. }
                    | PlanNodeKind::Driver { .. } => target.kind.is_value(),
                    PlanNodeKind::Composition { .. } => target.kind.is_gesture(),
                    PlanNodeKind::SharedValue { .. } | PlanNodeKind::Recognizer { .. } => false,
                };
                if !compatible {
                    return Err(PlanError::WrongDependencyKind);
                }
            }
            if let PlanNodeKind::PropertyBinding { sink, .. } = &node.kind {
                if !sinks.insert((node.node_id, *sink)) {
                    return Err(PlanError::DuplicateSink);
                }
            }
        }

        self.check_acyclic(&by_id)
    }

    fn check_acyclic(&self, by_id: &BTreeMap<u64, &PlanNode>) -> Result<(), PlanError> {
        // Iterative depth-first search with an explicit visit state, so a
        // hostile plan cannot exhaust the stack before it is refused.
        let mut state = BTreeMap::<u64, u8>::new();
        for root in by_id.keys().copied() {
            if state.get(&root) == Some(&2) {
                continue;
            }
            state.insert(root, 1);
            let mut stack = vec![(root, 0_usize)];
            while let Some((id, next)) = stack.last_mut() {
                let dependencies = by_id
                    .get(id)
                    .ok_or(PlanError::DanglingDependency)?
                    .kind
                    .dependencies();
                if *next == dependencies.len() {
                    state.insert(*id, 2);
                    stack.pop();
                    continue;
                }
                let child = dependencies[*next];
                *next += 1;
                match state.get(&child).copied().unwrap_or(0) {
                    0 => {
                        state.insert(child, 1);
                        stack.push((child, 0));
                    }
                    1 => return Err(PlanError::DependencyCycle),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// The value a command applies to.
///
/// The target is a plan identity, not a slab slot: the consumer resolves the
/// physical handle only after the generation and epoch fence succeeds, so a
/// command written against a replaced plan can never land on its successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTarget {
    /// Plan id of the value.
    pub value: u64,
    /// Plan generation the command was written against.
    pub generation: u32,
    /// Slab epoch the command was written against.
    pub epoch: u32,
}

/// What a command does to its target.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueCommandKind {
    /// Publish a scalar, cancelling any driver that owned the value.
    Write {
        /// The value to publish.
        value: f64,
    },
    /// Publish a scalar owned by an active gesture.
    GestureWrite {
        /// The value to publish.
        value: f64,
    },
    /// Start a driver, replacing any driver that owned the value.
    StartDriver {
        /// The animation to run.
        spec: AnimationDriverSpec,
        /// What to do instead when reduced motion is in effect.
        reduced_motion: ReducedMotionAction,
        /// Whether the new driver starts from the value's current velocity
        /// rather than from rest. This is what keeps a retarget continuous.
        inherit_velocity: bool,
    },
    /// End driver ownership without publishing anything further.
    CancelDriver,
}

/// One imperative operation against the value plane.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueCommand {
    /// Monotonic per-root ordering. Commands apply in this order or not at all.
    pub sequence: u64,
    /// The value this command applies to.
    pub target: CommandTarget,
    /// What to do.
    pub kind: ValueCommandKind,
}

impl ValueCommand {
    /// Check the command's identity and payload.
    pub fn validate(&self) -> Result<(), PlanError> {
        if self.sequence == 0
            || self.target.value == 0
            || self.target.generation == 0
            || self.target.epoch == 0
        {
            return Err(PlanError::ZeroIdentity);
        }
        match &self.kind {
            ValueCommandKind::Write { value } | ValueCommandKind::GestureWrite { value } => {
                if !value.is_finite() {
                    return Err(PlanError::NonFiniteValue);
                }
            }
            ValueCommandKind::StartDriver { .. } | ValueCommandKind::CancelDriver => {}
        }
        Ok(())
    }
}

/// What one outcome record is about.
///
/// The identity is what makes an outcome idempotent: a consumer that sees the
/// same identity twice has seen the same outcome twice, whatever the transport
/// did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OutcomeIdentity {
    /// One resolution of one pointer stream.
    Gesture {
        /// The pointer stream.
        stream_id: u64,
        /// Which resolution of that stream, from one.
        resolution_seq: u32,
    },
    /// One driver's end, on one tenant of one value.
    Driver {
        /// Slab slot of the value.
        value_slot: u32,
        /// Lifetime generation of the tenant.
        value_generation: u32,
        /// The driver's sequence within its table.
        driver_seq: u64,
    },
    /// One phase of one navigation transition.
    Navigation {
        /// The transition.
        transition_id: u64,
        /// Which phase of it.
        phase: u8,
    },
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionOutcome {
    /// A gesture won its stream and began emitting.
    GestureStart {
        /// Root gesture that won.
        root_gesture_id: u64,
        /// View node it is mounted on.
        node_id: u32,
    },
    /// A gesture reached its terminal.
    GestureResolution {
        /// Root gesture that resolved.
        root_gesture_id: u64,
        /// Whether the gesture committed rather than cancelled or failed.
        committed: bool,
    },
    /// A driver stopped owning its value.
    DriverTerminal(DriverTerminal),
    /// A navigation transition began.
    NavigationStart {
        /// The transition.
        transition_id: u64,
    },
}

/// One record in the evaluator's outcome stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutcomeRecord {
    /// Monotonic position in the stream, so a consumer can resume.
    pub cursor: u64,
    /// Slab epoch this outcome belongs to.
    pub epoch: u32,
    /// Motion root this outcome belongs to.
    pub root_instance: u32,
    /// What the outcome is about.
    pub identity: OutcomeIdentity,
    /// What happened.
    pub outcome: MotionOutcome,
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
