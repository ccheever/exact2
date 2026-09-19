use super::*;

/// A presentation attachment. Its Transform is the fallback when the target is unavailable.
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
pub(super) type SocketCache =
    BTreeMap<(String, String), (std::sync::Weak<Model>, Result<u32, String>)>;
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
        .ok_or_else(|| format!("socket model `{name}` not loaded"))?;
    let runtime = w.derived::<Runtime>();
    let mut cache = runtime.sockets.borrow_mut();
    let cached = cache
        .entry((name.clone(), joint.into()))
        .or_insert_with(|| {
            (
                std::sync::Arc::downgrade(model),
                named_node(model, joint).ok_or_else(|| format!("unknown socket `{joint}`")),
            )
        });
    if !cached
        .0
        .upgrade()
        .is_some_and(|old| std::sync::Arc::ptr_eq(&old, model))
    {
        *cached = (
            std::sync::Arc::downgrade(model),
            named_node(model, joint).ok_or_else(|| format!("unknown socket `{joint}`")),
        );
    }
    cached.1.clone()
}
/// Current tick-boundary socket in world space, composed after any game-authored movement.
/// Presentation interpolates the local chain separately at the displayed alpha.
pub fn socket(w: &World, target: impl crate::Target, joint: &str) -> Result<Transform, String> {
    let e = target.entity(w).ok_or("socket target does not exist")?;
    let node = socket_node(w, e, joint)?;
    let mesh = w.get::<Mesh>(e).unwrap();
    let Mesh::Asset(name) = &*mesh else {
        unreachable!()
    };
    let model = w.model(name).unwrap();
    let pose = w.get::<Pose>(e);
    let rest;
    let local = if let Some(pose) = &pose {
        if pose.local.len() != model.nodes.len() * 10 {
            return Err(format!("saved pose does not match model `{name}`"));
        }
        &pose.local
    } else {
        rest = bind_pose(model);
        &rest
    };
    let global = w
        .current_global(e)
        .ok_or("socket target has no Transform")?;
    let (scale, rotation, position) =
        (Mat4::from(global) * joint_matrix(model, local, node)).to_scale_rotation_translation();
    Ok(Transform {
        scale,
        rotation,
        position,
    })
}
