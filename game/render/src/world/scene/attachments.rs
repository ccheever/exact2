//! Socket attachments: followers posed on an animated owner's joints, composed
//! as full affine chains at frame alpha.
use super::{pose, snap, History};
use exact_game::{Entity, Parent, Transform, World};
use glam::{Mat4, Vec3};

/// One displayed attachment plus the ordinary histories restored after submission.
#[derive(Clone, Copy)]
pub struct DisplayedAttachment {
    /// Entity whose drawing, lights and placed children use this pose.
    pub entity: Entity,
    /// World pose composed from interpolated local joints.
    pub pose: Transform,
    /// Full affine map; geometry must retain this instead of decomposing shear.
    pub matrix: Mat4,
}
// Owner transforms must be interpolated locally before hierarchy composition:
// decomposing a global matrix loses shear under non-uniform ancestors.
struct Owner {
    // First valid follower this feed; emits the owner override in item order.
    first_attachment: Option<u32>,
    chain: Vec<History>,
    // Each chain link's presentation Offset, leaf to root: socketed props
    // follow the drawn rig, drawn(parent) · local · offset at every link.
    offsets: Vec<History>,
}
/// An entity's presentation offset, or the identity.
fn offset_of(w: &World, e: Entity) -> Transform {
    w.get::<exact_game::Offset>(e)
        .map_or_else(Transform::default, |o| o.0)
}
impl Owner {
    fn new(w: &World, entity: Entity) -> Self {
        let mut owner = Self {
            first_attachment: None,
            chain: Vec::new(),
            offsets: Vec::new(),
        };
        owner.update(w, entity, false, true);
        owner
    }
    fn update(&mut self, w: &World, entity: Entity, next_tick: bool, parent_changed: bool) {
        // Keep leaf-to-root history in place; ancestor edits snap the changed chain.
        let mut at = entity;
        let mut length = 0;
        for _ in 0..=w.len() {
            let curr = w
                .get::<Transform>(at)
                .as_deref()
                .copied()
                .unwrap_or_default();
            let offset = offset_of(w, at);
            for (list, value) in [(&mut self.chain, curr), (&mut self.offsets, offset)] {
                if list.get(length).is_none_or(|h| h.entity != at) {
                    if let Some(found) = list[length..].iter().position(|h| h.entity == at) {
                        list.swap(length, length + found);
                    } else {
                        list.insert(length, History::new(at, value));
                    }
                }
                let h = &mut list[length];
                if next_tick {
                    h.prev = h.curr;
                }
                h.curr = value;
                if snap(w, at, parent_changed) {
                    h.prev = value;
                }
            }
            length += 1;
            let Some(parent) = w.get::<Parent>(at).map(|p| p.0).filter(|e| w.contains(*e)) else {
                break;
            };
            at = parent;
        }
        self.chain.truncate(length);
        self.offsets.truncate(length);
    }
}
#[derive(Default)]
pub(crate) struct DiagnosticRegistry {
    world: Option<exact_game::WorldId>,
    messages: std::collections::BTreeMap<(Entity, &'static str), String>,
}
impl std::ops::Deref for DiagnosticRegistry {
    type Target = std::collections::BTreeMap<(Entity, &'static str), String>;
    fn deref(&self) -> &Self::Target {
        &self.messages
    }
}
impl std::ops::DerefMut for DiagnosticRegistry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.messages
    }
}
impl DiagnosticRegistry {
    pub(crate) fn restored(&mut self, w: &World) {
        self.world = Some(w.id());
    }
    pub(crate) fn observe(&mut self, w: &World) {
        if self.world.as_ref() != Some(&w.id()) {
            self.messages.clear();
            self.restored(w);
        }
        self.messages.retain(|(e, _), _| w.contains(*e));
    }
}
pub(crate) type AttachmentDiagnostics = std::rc::Rc<std::cell::RefCell<DiagnosticRegistry>>;
struct Attachment {
    history: History,
    owner: Entity,
    chain: Vec<[Transform; 2]>,
    offset: Transform,
    // The follower's own presentation Offset, after the socket's.
    drawn: History,
    model_digest: u64,
}
#[derive(Default)]
pub(crate) struct Attachments {
    owners: std::collections::BTreeMap<Entity, Owner>,
    // Feed keeps one generation per entity index, in index order.
    items: Vec<Attachment>,
    pub(crate) diagnostics: AttachmentDiagnostics,
    pub output: Vec<DisplayedAttachment>,
    pub(crate) model_digests: std::collections::BTreeMap<String, u64>,
    // An owner's presentation playback (`animation::ShownClips`), sampled per follower.
    shown: exact_game::animation::ShownPose,
}
impl Attachments {
    pub fn reset(&mut self) {
        self.owners.clear();
        self.items.clear();
        self.output.clear();
    }
    fn warn(&mut self, e: Entity, identity: &'static str, message: impl FnOnce() -> String) {
        let message = message();
        let mut diagnostics = self.diagnostics.borrow_mut();
        if diagnostics.get(&(e, identity)) == Some(&message) {
            return;
        }
        // Host-only diagnostics never enter continuation saves.
        #[cfg(target_arch = "wasm32")]
        web_sys::console::warn_1(&message.clone().into());
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("{message}");
        diagnostics.insert((e, identity), message);
    }

    pub fn feed(
        &mut self,
        w: &World,
        initial: bool,
        next_tick: bool,
        parent_changed: bool,
        models_changed: bool,
    ) {
        if initial {
            self.owners.clear();
        }
        self.owners.retain(|e, _| w.contains(*e));
        self.diagnostics.borrow_mut().observe(w);
        for (&entity, owner) in &mut self.owners {
            owner.first_attachment = None;
            owner.update(w, entity, next_tick, parent_changed);
        }
        // Retain animated owners before attachments are spawned so a new charm
        // inherits the owner's displayed history, rather than snapping its bones.
        for (e, _) in w.query::<&exact_game::Pose>().iter() {
            if w.has::<Transform>(e) {
                self.owners.entry(e).or_insert_with(|| Owner::new(w, e));
            }
        }
        let mut at = 0;
        for (e, follow) in w.query::<&exact_game::SocketFollow>().iter() {
            let target = match &follow.target {
                exact_game::FollowTarget::Entity(e) => Some(*e),
                exact_game::FollowTarget::Name(n) => w.named(n),
            };
            while at < self.items.len() && self.items[at].history.entity.index() < e.index() {
                self.items.remove(at);
            }
            let resolved = target
                .ok_or_else(|| format!("unresolved target {:?}", follow.target))
                .and_then(|target| {
                    exact_game::animation::socket_node(w, target, &follow.joint)
                        .map(|node| (target, node))
                });
            let (target, node) = match resolved {
                Ok(target) => target,
                Err(error) => {
                    self.warn(e, "socket", || {
                        format!(
                            "SocketFollow `{}`: {error}; using authored Transform",
                            w.name(e).unwrap_or("unnamed")
                        )
                    });
                    continue;
                }
            };
            let (Some(home), Some(_)) = (pose(w, e), w.get::<Transform>(target)) else {
                self.warn(e, "transform", || {
                    format!(
                        "SocketFollow `{}`: unresolved Transform",
                        w.name(e).unwrap_or("unnamed")
                    )
                });
                continue;
            };
            let Some(mesh) = w.get::<exact_game::Mesh>(target) else {
                continue;
            };
            let exact_game::Mesh::Asset(name) = &*mesh else {
                continue;
            };
            // Drawn sockets follow the drawn rig: any arrived model, so a
            // streamed owner's `ShownClips` pose carries its props too.
            let Some(model) = exact_game::animation::drawn_model(w, name) else {
                continue;
            };
            let fresh = initial
                || !self
                    .items
                    .get(at)
                    .is_some_and(|v| v.history.entity == e && v.owner == target);
            if fresh {
                if self.items.get(at).is_some_and(|v| v.history.entity == e) {
                    self.items.remove(at);
                }
                self.items.insert(
                    at,
                    Attachment {
                        history: History::new(e, home),
                        owner: target,
                        chain: vec![],
                        offset: follow.offset,
                        drawn: History::new(e, offset_of(w, e)),
                        model_digest: self
                            .model_digests
                            .get(name)
                            .copied()
                            .unwrap_or(w.model_revision()),
                    },
                );
            }
            let item = &mut self.items[at];
            item.history
                .update_to(home, next_tick, snap(w, e, parent_changed));
            item.drawn
                .update_to(offset_of(w, e), next_tick, snap(w, e, parent_changed));
            self.owners
                .entry(target)
                .or_insert_with(|| Owner::new(w, target))
                .first_attachment
                .get_or_insert(e.index());
            let model_changed = if models_changed {
                let digest = self
                    .model_digests
                    .get(name)
                    .copied()
                    .unwrap_or(w.model_revision());
                let changed = item.model_digest != digest;
                item.model_digest = digest;
                changed
            } else {
                false
            };
            item.offset = follow.offset;
            item.chain.clear();
            let pose = w.get::<exact_game::Pose>(target);
            let shown = w.has::<exact_game::animation::ShownClips>(target)
                && (self.shown)
                    .sample(w, target, &exact_game::animation::bind_pose(model))
                    .unwrap_or(false);
            let sampled = if shown {
                Some((&self.shown.previous[..], &self.shown.local[..]))
            } else {
                pose.as_ref()
                    .map(|p| (&p.previous[..], &p.local[..]))
                    .filter(|(previous, local)| {
                        local.len() == model.nodes.len() * 10 && previous.len() == local.len()
                    })
            };
            let snap = initial
                || model_changed
                || exact_game::animation::socket_stale(w, target)
                || snap(w, target, parent_changed);
            let nodes = std::iter::successors(Some(node), |&i| model.nodes[i as usize].parent);
            if let Some((previous, local)) = sampled {
                let prev = if snap { local } else { previous };
                item.chain.extend(nodes.map(|i| {
                    let start = i as usize * 10;
                    let read = |values: &[f32]| Transform {
                        position: Vec3::from_slice(&values[start..]),
                        rotation: glam::Quat::from_slice(&values[start + 3..]).normalize(),
                        scale: Vec3::from_slice(&values[start + 7..]),
                    };
                    [read(prev), read(local)]
                }));
            } else {
                item.chain.extend(nodes.map(|i| {
                    let (scale, rotation, position) =
                        Mat4::from_cols_array(&model.nodes[i as usize].transform)
                            .to_scale_rotation_translation();
                    // Match bind_pose's normalization followed by the pose reader's,
                    // without allocating or decomposing unrelated model nodes.
                    [Transform {
                        position,
                        rotation: rotation.normalize().normalize(),
                        scale,
                    }; 2]
                }));
            }
            item.chain.reverse();
            at += 1;
        }
        self.items.truncate(at);
        self.output.clear();
        self.output.reserve(self.items.len());
    }
    fn matrix(&self, index: usize, alpha: f32, remaining: usize) -> Mat4 {
        let item = &self.items[index];
        if remaining == 0 {
            return matrix(item.history.at(alpha));
        }
        let owner = self.owner_matrix(&self.owners[&item.owner], alpha, remaining);
        item.chain.iter().fold(owner, |m, pair| {
            m * crate::skinning::interpolated_local(*pair, alpha)
        }) * matrix(item.offset)
            * matrix(item.drawn.at(alpha))
    }
    fn owner_matrix(&self, owner: &Owner, alpha: f32, remaining: usize) -> Mat4 {
        // A link that is itself a follower is already drawn, offset included.
        let links = owner.chain.iter().zip(&owner.offsets).rev();
        links.fold(Mat4::IDENTITY, |m, (h, offset)| {
            self.items
                .binary_search_by_key(&h.entity, |v| v.history.entity)
                .ok()
                .map_or_else(
                    || m * matrix(h.at(alpha)) * matrix(offset.at(alpha)),
                    |i| self.matrix(i, alpha, remaining - 1),
                )
        })
    }
    pub fn frame(&mut self, alpha: f32) {
        self.output.clear();
        for (i, item) in self.items.iter().enumerate() {
            let matrix = self.matrix(i, alpha, self.items.len());
            let (scale, rotation, position) = matrix.to_scale_rotation_translation();
            self.output.push(DisplayedAttachment {
                entity: item.history.entity,
                matrix,
                pose: Transform {
                    scale,
                    rotation,
                    position,
                },
            });
        }
        // The skinned owner must use the same affine chain as its charm; otherwise
        // the owner mesh would still flatten ancestor shear in its TRS arena.
        for item in &self.items {
            let owner = &self.owners[&item.owner];
            if owner.chain.len() < 2
                || owner.first_attachment != Some(item.history.entity.index())
                || self
                    .items
                    .binary_search_by_key(&item.owner, |v| v.history.entity)
                    .is_ok()
            {
                continue;
            }
            let matrix = self.owner_matrix(owner, alpha, self.items.len());
            let (scale, rotation, position) = matrix.to_scale_rotation_translation();
            self.output.push(DisplayedAttachment {
                entity: item.owner,
                matrix,
                pose: Transform {
                    scale,
                    rotation,
                    position,
                },
            });
        }
    }
}
fn matrix(t: Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}
pub(crate) fn displayed(
    attachments: &[DisplayedAttachment],
    entity: Entity,
    fallback: Transform,
) -> Transform {
    attachments
        .iter()
        .find(|v| v.entity == entity)
        .map_or(fallback, |v| v.pose)
}

pub(crate) fn displayed_matrix(
    attachments: &[DisplayedAttachment],
    entity: Entity,
    fallback: Transform,
) -> Mat4 {
    attachments
        .iter()
        .find(|a| a.entity == entity)
        .map_or_else(|| matrix(fallback), |a| a.matrix)
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    struct Rig;
    impl exact_game::Game for Rig {
        const ID: &'static str = "attachment-owner";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &exact_game::Input, _: &()) {}
    }
    #[test]
    fn attachments_share_one_owner_and_the_delivered_model_identity() {
        let model = crate::test_model::skinned_model();
        let before = crate::models::model_hash_count();
        let digest = crate::models::model_digest(&model);
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset("rig.model", Ok(exact_game::asset::Content::Model(model)))
            .unwrap();
        let w = sim.world_mut();
        let owner = w.spawn_named(
            "rig",
            (Transform::default(), exact_game::Mesh::asset("rig.model")),
        );
        for _ in 0..3 {
            w.spawn((
                Transform::default(),
                exact_game::SocketFollow::new("rig", "joint"),
            ));
        }
        w.propagate();
        let mut a = Attachments::default();
        a.model_digests.insert("rig.model".into(), digest);
        a.feed(w, true, true, true, true);
        assert_eq!(a.owners.len(), 1);
        assert_eq!(a.items.len(), 3);
        assert!(a
            .items
            .iter()
            .all(|v| v.owner == owner && v.model_digest == digest));
        // A residency identity is supplied, never rediscovered from model bytes.
        a.model_digests.insert("rig.model".into(), digest ^ 1);
        a.feed(w, false, true, false, true);
        assert!(a.items.iter().all(|v| v.model_digest == digest ^ 1));
        assert_eq!(crate::models::model_hash_count(), before + 1);
    }

    #[test]
    fn bind_fallback_matches_sampled_rest_for_affine_joint_chains() {
        use exact_game::{asset, Mesh, Pose, SocketFollow};
        let model = asset::Model {
            nodes: (0..16)
                .map(|i| {
                    let mut transform = Mat4::from_scale_rotation_translation(
                        Vec3::new(if i % 3 == 0 { -1.1 } else { 1.1 }, 0.9, 1.),
                        glam::Quat::from_rotation_y(i as f32 * 0.173),
                        Vec3::new(0.1, i as f32 * 0.02, -0.03),
                    );
                    transform.y_axis += transform.x_axis * 0.15;
                    asset::Node {
                        name: format!("joint-{i}"),
                        parent: (i > 0 && i < 10).then(|| i - 1),
                        transform: transform.to_cols_array(),
                        ..Default::default()
                    }
                })
                .collect(),
            ..Default::default()
        };
        let rest = exact_game::animation::bind_pose(&model);
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset("rig.model", Ok(asset::Content::Model(model)))
            .unwrap();
        let w = sim.world_mut();
        let owner = w.spawn((Transform::at(2., 1., 0.), Mesh::asset("rig.model")));
        w.spawn((
            Transform::default(),
            SocketFollow::new(owner, "joint-9").offset(Transform::at(0.2, 0.3, 0.)),
        ));
        let mut pose = Pose::default();
        pose.local = rest.clone();
        pose.previous = rest;
        w.insert(owner, pose.clone());
        let capture = |w: &World| {
            let saved = w.save();
            let mut attachments = Attachments::default();
            attachments.feed(w, true, true, true, true);
            assert_eq!(attachments.items[0].chain.len(), 10);
            let mut bits = Vec::new();
            for pair in &attachments.items[0].chain {
                for t in pair {
                    bits.extend(t.position.to_array().map(f32::to_bits));
                    bits.extend(t.rotation.to_array().map(f32::to_bits));
                    bits.extend(t.scale.to_array().map(f32::to_bits));
                }
            }
            for alpha in [0., 0.25, 0.75, 1.] {
                attachments.frame(alpha);
                bits.extend(
                    attachments.output[0]
                        .matrix
                        .to_cols_array()
                        .map(f32::to_bits),
                );
            }
            assert_eq!(w.save(), saved);
            bits
        };
        let expected = capture(w);
        w.remove::<Pose>(owner);
        assert_eq!(capture(w), expected);
        pose.previous.pop();
        w.insert(owner, pose.clone());
        assert_eq!(capture(w), expected);
        pose.local.pop();
        w.insert(owner, pose);
        assert_eq!(capture(w), expected);
    }

    #[test]
    fn attachment_lookup_keeps_entity_order_and_rejects_reused_slots() {
        use exact_game::{FollowTarget, Mesh, SocketFollow};
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset(
            "rig.model",
            Ok(exact_game::asset::Content::Model(
                crate::test_model::skinned_model(),
            )),
        )
        .unwrap();
        let w = sim.world_mut();
        let root = w.spawn((Transform::at(10., 0., 0.), Mesh::asset("rig.model")));
        let old = w.spawn((
            Transform::at(77., 0., 0.),
            Mesh::asset("rig.model"),
            SocketFollow::new(root, "joint"),
        ));
        let stale = Owner::new(w, old);
        let middle = w.spawn((Transform::default(), SocketFollow::new(root, "joint")));
        let tail = w.spawn((Transform::default(), SocketFollow::new(old, "joint")));
        w.propagate();
        let mut a = Attachments::default();
        a.feed(w, true, true, true, true);
        a.frame(0.5);
        assert_eq!(
            displayed(&a.output, tail, Transform::default()).position.x,
            10.
        );

        w.despawn(old);
        let mut socket = SocketFollow::new(root, "joint");
        socket.offset = Transform::at(4., 0., 0.);
        let replacement = w.spawn((Transform::at(3., 0., 0.), Mesh::asset("rig.model"), socket));
        assert_eq!(replacement.index(), old.index());
        assert_ne!(replacement, old);
        w.get_mut::<SocketFollow>(tail).unwrap().target = FollowTarget::Entity(replacement);
        w.get_mut::<SocketFollow>(middle).unwrap().joint = "missing".into();
        w.propagate();
        a.feed(w, false, true, false, false);
        assert_eq!(
            a.items.iter().map(|v| v.history.entity).collect::<Vec<_>>(),
            [replacement, tail]
        );
        a.frame(0.5);
        assert_eq!(
            displayed(&a.output, tail, Transform::default()).position.x,
            14.
        );
        assert_eq!(
            a.owner_matrix(&stale, 0.5, a.items.len()),
            matrix(Transform::at(77., 0., 0.))
        );

        w.get_mut::<SocketFollow>(middle).unwrap().joint = "joint".into();
        w.remove::<SocketFollow>(replacement);
        a.feed(w, false, false, false, false);
        assert_eq!(
            a.items.iter().map(|v| v.history.entity).collect::<Vec<_>>(),
            [middle, tail]
        );
        a.frame(0.5);
        assert_eq!(
            displayed(&a.output, tail, Transform::default()).position.x,
            3.
        );
    }

    #[test]
    fn parented_owner_overrides_follow_current_attachments_once() {
        use exact_game::{Mesh, SocketFollow};
        let mut sim = exact_game::Sim::<Rig>::new(()).unwrap();
        sim.deliver_asset(
            "rig.model",
            Ok(exact_game::asset::Content::Model(
                crate::test_model::skinned_model(),
            )),
        )
        .unwrap();
        let w = sim.world_mut();
        let parent = w.spawn(Transform::at(100., 0., 0.));
        let a = w.spawn((
            Transform::at(1., 0., 0.),
            Parent(parent),
            Mesh::asset("rig.model"),
        ));
        let b = w.spawn((
            Transform::at(2., 0., 0.),
            Parent(parent),
            Mesh::asset("rig.model"),
        ));
        let first_b = w.spawn((Transform::default(), SocketFollow::new(b, "joint")));
        let first_a = w.spawn((Transform::default(), SocketFollow::new(a, "joint")));
        let last_b = w.spawn((Transform::default(), SocketFollow::new(b, "joint")));
        let mut attachments = Attachments::default();
        let mut check = |w: &mut World, expected: &[Entity]| {
            w.propagate();
            attachments.feed(w, false, true, true, false);
            for alpha in [0., 0.5, 1.] {
                attachments.frame(alpha);
                assert_eq!(
                    attachments
                        .output
                        .iter()
                        .map(|v| v.entity)
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        };
        // Owner order follows the first referring attachment, not the map key.
        check(w, &[first_b, first_a, last_b, b, a]);
        w.despawn(first_b);
        check(w, &[first_a, last_b, a, b]);
        w.get_mut::<SocketFollow>(first_a).unwrap().joint = "missing".into();
        check(w, &[last_b, b]);
        w.get_mut::<SocketFollow>(first_a).unwrap().joint = "joint".into();
        check(w, &[first_a, last_b, a, b]);
        w.remove::<Parent>(b);
        check(w, &[first_a, last_b, a]);
        // An owner that is itself attached already has its composed override.
        w.insert(a, SocketFollow::new(b, "joint"));
        check(w, &[a, first_a, last_b]);
        w.remove::<SocketFollow>(first_a);
        w.remove::<SocketFollow>(last_b);
        check(w, &[a]);
        w.remove::<SocketFollow>(a);
        check(w, &[]);
    }

    #[test]
    fn owner_updates_reuse_one_chain_allocation() {
        let mut w = World::new(60, 0);
        let parent = w.spawn(Transform::at(2., 0., 0.));
        let child = w.spawn((Transform::at(1., 0., 0.), Parent(parent)));
        let mut owner = Owner::new(&w, child);
        let allocation = owner.chain.as_ptr();
        for _ in 0..100 {
            owner.update(&w, child, true, false);
            assert_eq!(owner.chain.as_ptr(), allocation);
            assert_eq!(owner.chain.len(), 2);
        }
        w.remove::<Parent>(child);
        owner.update(&w, child, true, true);
        assert_eq!(owner.chain.len(), 1);
        assert_eq!(owner.chain.as_ptr(), allocation);
    }

    #[test]
    fn distinct_diagnostic_identities_do_not_replace_each_other() {
        let mut w = World::new(60, 0);
        let e = w.spawn(Transform::default());
        let mut a = Attachments::default();
        a.warn(e, "socket", || "missing joint".into());
        a.warn(e, "stale", || "stale pose".into());
        a.warn(e, "socket", || "missing target".into());
        a.warn(e, "stale", || "stale pose".into());
        let registry = a.diagnostics.borrow();
        assert_eq!(registry.len(), 2);
        assert_eq!(registry[&(e, "socket")], "missing target");
        assert_eq!(registry[&(e, "stale")], "stale pose");
    }
}
