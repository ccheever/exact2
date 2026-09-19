use super::*;

/// A tick-boundary and presentation attachment. Its Transform is the fallback when the target is unavailable.
/// Each attachment chooses its own joint; no bone entities or simulation transform writes.
#[derive(Default, Clone, Debug, Data)]
pub struct SocketFollow {
    pub target: crate::FollowTarget,
    pub joint: String,
    pub offset: Transform,
}
impl SocketFollow {
    pub fn new(target: impl Into<crate::FollowTarget>, joint: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            joint: joint.into(),
            ..Self::default()
        }
    }
    pub fn offset(mut self, offset: Transform) -> Self {
        self.offset = offset;
        self
    }
}
/// Owned playback output; reading it holds no world/component leases.
#[derive(Default, Clone)]
pub struct Motion(pub(super) Vec<(Entity, Option<String>, Playback)>);
impl Motion {
    fn playback(&self, target: impl Into<crate::FollowTarget>) -> Option<&Playback> {
        let target = target.into();
        self.0
            .iter()
            .find(|(e, name, _)| match &target {
                crate::FollowTarget::Entity(id) => e == id,
                crate::FollowTarget::Name(n) => name.as_ref() == Some(n),
            })
            .map(|(_, _, p)| p)
    }
    pub fn root_motion(&self, target: impl Into<crate::FollowTarget>) -> Vec3 {
        self.playback(target)
            .map_or(Vec3::ZERO, Playback::root_motion)
    }
    pub fn crossed(&self, target: impl Into<crate::FollowTarget>, marker: &str) -> bool {
        self.playback(target).is_some_and(|p| p.crossed(marker))
    }
}
pub(super) struct ModelSockets {
    model: std::sync::Weak<Model>,
    joints: BTreeMap<String, Result<u32, String>>,
}
pub(super) type SocketCache = BTreeMap<String, ModelSockets>;
/// Resolve a joint per loaded rig, including cached named refusals.
pub fn socket_node(w: &World, target: impl crate::Target, joint: &str) -> Result<u32, String> {
    let e = target.entity(w).ok_or("socket target does not exist")?;
    let mesh = w.get::<Mesh>(e).ok_or("socket target has no Mesh")?;
    let Mesh::Asset(name) = &*mesh else {
        return Err("socket target needs Mesh::asset".into());
    };
    let model = w
        .assets
        .models
        .get(name)
        .map(|asset| &asset.model)
        .ok_or_else(|| format!("socket model `{name}` not loaded"))?;
    let mut runtime = w.derived::<Runtime>();
    let cache = &mut runtime.sockets;
    if !cache.get(name).is_some_and(|cached| {
        cached
            .model
            .upgrade()
            .is_some_and(|old| std::sync::Arc::ptr_eq(&old, model))
    }) {
        cache.insert(
            name.clone(),
            ModelSockets {
                model: std::sync::Arc::downgrade(model),
                joints: BTreeMap::new(),
            },
        );
    }
    let joints = &mut cache.get_mut(name).unwrap().joints;
    if let Some(node) = joints.get(joint) {
        return node.clone();
    }
    let node = named_node(model, joint).ok_or_else(|| format!("unknown socket `{joint}`"));
    joints.insert(joint.into(), node.clone());
    node
}
/// Current tick-boundary socket in world space. Call `step`, apply its Motion to
/// the owner Transform, then query sockets. Presentation samples locals at frame alpha.
/// Use [`socket_matrix`] when the rig can produce shear.
pub fn socket(w: &World, target: impl crate::Target, joint: &str) -> Result<Transform, String> {
    let (scale, rotation, position) =
        socket_matrix(w, target, joint)?.to_scale_rotation_translation();
    Ok(Transform {
        scale,
        rotation,
        position,
    })
}
/// Full affine tick-boundary socket, preserving shear from non-uniform ancestors.
pub fn socket_matrix(w: &World, target: impl crate::Target, joint: &str) -> Result<Mat4, String> {
    let e = target
        .entity(w)
        .ok_or_else(|| format!("socket target does not exist: `{}`", target.label()))?;
    socket_matrix_with(w, e, joint, w.len() + 1)
}
pub(crate) fn socket_matrix_with(
    w: &World,
    e: Entity,
    joint: &str,
    remaining: usize,
) -> Result<Mat4, String> {
    let label = || {
        w.name(e)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("#{}", e.index()))
    };
    if remaining == 0 {
        return Err(format!("socket target `{}` has a follower cycle", label()));
    }
    if socket_stale(w, e) {
        return Err(format!("socket target `{}` has a stale pose; call animation::step(w), apply Motion, then query socket", label()));
    }
    compose_socket(w, e, joint, remaining)
}
/// Whether an animated socket needs this tick's animation step. Setup's bind pose is valid.
pub fn socket_stale(w: &World, e: Entity) -> bool {
    if !(w.has::<Animation>(e) || w.has::<Blend>(e) || w.has::<Animator>(e)) {
        return false;
    }
    let stepped = w.get::<Pose>(e).and_then(|p| p.stepped);
    !(stepped == Some(w.tick())
        || (!w.in_tick
            && ((w.tick() == 0 && stepped.is_none())
                || w.tick()
                    .checked_sub(1)
                    .is_some_and(|tick| stepped == Some(tick)))))
}
fn compose_socket(w: &World, e: Entity, joint: &str, remaining: usize) -> Result<Mat4, String> {
    if remaining == 0 {
        return Err("socket follower cycle".into());
    }
    let label = w.name(e).unwrap_or("unnamed");
    let pose = w.get::<Pose>(e);
    let node = socket_node(w, e, joint)?;
    let mesh = w.get::<Mesh>(e).unwrap();
    let Mesh::Asset(name) = &*mesh else {
        unreachable!()
    };
    let model = w
        .model(name)
        .ok_or_else(|| format!("socket target `{label}` needs declared model `{name}`"))?;
    let rest;
    let local = if let Some(pose) = &pose {
        if pose.local.len() != model.nodes.len() * 10 {
            return Err(format!(
                "saved pose on `{label}` does not match model `{name}`"
            ));
        }
        &pose.local
    } else {
        rest = bind_pose(model);
        &rest
    };
    let global = w.current_global_depth(e, remaining - 1).ok_or_else(|| {
        format!("socket target `{label}` has no Transform or has a follower cycle")
    })?;
    Ok(Mat4::from(global) * joint_matrix(model, local, node))
}

impl Component for SocketFollow {
    const NAME: &'static str = "SocketFollow";
    fn register(w: &mut World) {
        w.register::<Pose>();
        w.attachments = Some(crate::world::Attachments {
            pose: follower_pose,
            propagate: |w| {
                // Seed completed boundaries even when no presenter/audio query ran.
                prune_follower_poses(w);
                for (e, _) in w.query::<&SocketFollow>().iter() {
                    let _ = w.current_global(e);
                }
            },
        });
    }
}
fn prune_follower_poses(w: &World) {
    let stamp = (w.entities_revision(), w.membership::<SocketFollow>());
    let mut cache = w.derived::<FollowerPoses>();
    if cache.1 != Some(stamp) {
        cache
            .0
            .retain(|entity, _| w.contains(*entity) && w.has::<SocketFollow>(*entity));
        cache.1 = Some(stamp);
    }
}
fn follower_pose(w: &World, e: Entity, remaining: usize) -> Option<crate::Affine3A> {
    prune_follower_poses(w);
    let follow = w.get::<SocketFollow>(e)?;
    let target = match &follow.target {
        crate::FollowTarget::Entity(e) => *e,
        crate::FollowTarget::Name(n) => w.named(n)?,
    };
    if !w.contains(target) {
        return None;
    }
    if socket_stale(w, target) {
        if let Some((old, pose)) = w.derived::<FollowerPoses>().0.get(&e) {
            if *old == target {
                return Some(*pose);
            }
        }
    }
    // A restored stale boundary still has its saved local joint pose. Explicit
    // socket queries refuse it; followers can reconstruct without an authored snap.
    let socket = compose_socket(w, target, &follow.joint, remaining).ok()?;
    let pose = crate::Affine3A::from_mat4(socket)
        * crate::Affine3A::from_scale_rotation_translation(
            follow.offset.scale,
            follow.offset.rotation,
            follow.offset.position,
        );
    let mut cache = w.derived::<FollowerPoses>();
    cache.0.insert(e, (target, pose));
    Some(pose)
}
#[derive(Default)]
struct FollowerPoses(
    BTreeMap<Entity, (Entity, crate::Affine3A)>,
    Option<(u64, u64)>,
);

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn dead_followers_are_pruned_even_without_remaining_followers() {
        for restore in [false, true] {
            let mut w = World::new(60, 0);
            let target = w.spawn(Transform::default());
            let e = w.spawn(SocketFollow::new(target, ""));
            if restore {
                w.load(&w.save()).unwrap();
            }
            w.derived::<FollowerPoses>()
                .0
                .insert(e, (target, crate::Affine3A::IDENTITY));
            w.despawn(e);
            w.propagate();
            assert!(w.derived::<FollowerPoses>().0.is_empty());
        }
    }
    #[test]
    fn stale_hits_prune_dead_followers_before_returning() {
        let mut w = World::new(60, 0);
        let target = w.spawn((Transform::default(), Animation::play("idle")));
        let live = w.spawn(SocketFollow::new(target, ""));
        let dead = w.spawn(SocketFollow::new(target, ""));
        for e in [live, dead] {
            w.derived::<FollowerPoses>()
                .0
                .insert(e, (target, crate::Affine3A::IDENTITY));
        }
        w.despawn(dead);
        w.begin_tick();
        assert!(follower_pose(&w, live, 10).is_some());
        assert_eq!(w.derived::<FollowerPoses>().0.len(), 1);
    }
}
