//! The kernel→motion seam: a commit, restated as what the engine needs to hear.
//!
//! @ref LLP 1002 §3 (the frame); LLP 1003 §4 (the seam)
//!
//! Motion's inputs are style. After a commit, the animatable rows of every
//! node the commit created or touched are the engine's new targets, and each
//! node's `transition` row is how it gets there — exactly the two things a
//! browser reads from computed style. Nothing else crosses: no bindings, no
//! shared values, no second graph. A destroyed node is forgotten.
//!
//! The engine keys nodes by a number the host chooses; here it is the
//! generation-checked [`NodeKey`] packed into a `u64` ([`motion_node`]), so a
//! reused slot never inherits its predecessor's motion.

use crate::generated::StyleProps;
use crate::id::NodeKey;
use crate::kernel::Kernel;
use crate::txn::CommitReceipt;
use exact_motion::{Change, Engine, EngineError, Property, Transitions, Value};

/// The engine's node number for a kernel node.
pub fn motion_node(key: NodeKey) -> u64 {
    ((key.generation as u64) << 32) | key.index as u64
}

/// Everything the motion engine must hear about one commit, in the order it
/// must hear it: forgotten nodes, then per node its `transition` row, then its
/// targets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MotionSync {
    /// Nodes destroyed by the commit.
    pub removed: Vec<u64>,
    /// Each created or touched node's `transition` row.
    pub transitions: Vec<(u64, Transitions)>,
    /// Each created or touched node's animatable targets, every property.
    pub changes: Vec<Change>,
}

impl MotionSync {
    /// Feed the engine, in order.
    pub fn apply(&self, engine: &mut Engine) -> Result<(), EngineError> {
        for node in &self.removed {
            engine.remove(*node);
        }
        for (node, transitions) in &self.transitions {
            engine.set_transitions(*node, transitions.clone())?;
        }
        for change in &self.changes {
            engine.observe(*change)?;
        }
        Ok(())
    }
}

/// The animatable rows of one style, as engine values. CSS's own property
/// vocabulary: `translate` (two lengths), `scale`, `rotate` (degrees),
/// `opacity`.
pub fn targets(style: &StyleProps) -> [(Property, Value); 4] {
    [
        (
            Property::Translate,
            Value::new(style.translate.x as f64, style.translate.y as f64),
        ),
        (Property::Scale, Value::scalar(style.scale as f64)),
        (Property::Rotate, Value::scalar(style.rotate as f64)),
        (Property::Opacity, Value::scalar(style.opacity as f64)),
    ]
}

impl Kernel {
    /// Restate a commit for the motion engine. The receipt must be one this
    /// kernel produced; a key the commit destroyed resolves to nothing, which
    /// is exactly what makes it a removal.
    pub fn motion_sync(&self, receipt: &CommitReceipt) -> MotionSync {
        let mut sync = MotionSync {
            removed: receipt.destroyed.iter().copied().map(motion_node).collect(),
            ..MotionSync::default()
        };
        for key in receipt.created.iter().chain(receipt.touched.iter()) {
            let Some(node) = self.node_by_key(*key) else {
                continue;
            };
            let id = motion_node(*key);
            sync.transitions.push((id, node.style.transition.clone()));
            for (property, value) in targets(node.style) {
                sync.changes.push(Change {
                    node: id,
                    property,
                    value,
                    velocity: None,
                });
            }
        }
        sync
    }
}
