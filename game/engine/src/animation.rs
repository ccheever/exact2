//! Saved clip clocks and local poses. `step` is automatic at tick end, or explicit
//! before a game's marker/root-motion reads; a second call in one tick is a no-op.
#![allow(missing_docs)]
use crate::{
    asset::{Clip, Interpolation, Model, Track, TrackPath},
    math, Component, Data, Entity, Mesh, Quat, Transform, Vec3, World,
};
use glam::Mat4;
use std::{any::TypeId, collections::BTreeMap};

#[derive(Clone, Debug, Component)]
pub struct Animation {
    pub clip: String,
    pub time: f32,
    pub speed: f32,
    pub looping: bool,
    pub markers: Vec<(f32, String)>,
    crossed: Vec<String>,
    root_motion: Vec3,
}
impl Default for Animation {
    fn default() -> Self {
        Self {
            clip: String::new(),
            time: 0.,
            speed: 1.,
            looping: true,
            markers: vec![],
            crossed: vec![],
            root_motion: Vec3::ZERO,
        }
    }
}
impl Animation {
    pub fn play(clip: impl Into<String>) -> Self {
        Self {
            clip: clip.into(),
            ..Self::default()
        }
    }
    pub fn speed(mut self, speed: f32) -> Self {
        assert!(speed.is_finite());
        self.speed = speed;
        self
    }
    pub fn once(mut self) -> Self {
        self.looping = false;
        self
    }
    pub fn marker(mut self, seconds: f32, name: impl Into<String>) -> Self {
        assert!(seconds.is_finite() && seconds >= 0.);
        self.markers.push((seconds, name.into()));
        self
    }
    pub fn crossed(&self, name: &str) -> bool {
        self.crossed.iter().any(|n| n == name)
    }
    /// Model-local translation contributed this tick. The game decides whether to use it.
    pub fn root_motion(&self) -> Vec3 {
        self.root_motion
    }
}
#[derive(Default, Clone, Debug, Component)]
pub struct Blend {
    pub axis: f32,
    pub clips: Vec<(f32, String)>,
}
impl Blend {
    pub fn across<const N: usize>(clips: [(f32, &str); N]) -> Self {
        let mut clips: Vec<_> = clips.into_iter().map(|(v, n)| (v, n.into())).collect();
        clips.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(
            !clips.is_empty()
                && clips.iter().all(|c| c.0.is_finite())
                && clips.windows(2).all(|p| p[0].0 < p[1].0)
        );
        Self {
            axis: clips[0].0,
            clips,
        }
    }
    fn pair<'a>(&self, model: &'a Model) -> Result<(&'a Clip, &'a Clip, f32), String> {
        if !self.axis.is_finite()
            || self.clips.is_empty()
            || self.clips.iter().any(|c| !c.0.is_finite())
            || self.clips.windows(2).any(|p| p[0].0 >= p[1].0)
        {
            return Err("invalid blend axis/knots".into());
        }
        let hi = self
            .clips
            .partition_point(|p| p.0 < self.axis)
            .min(self.clips.len() - 1);
        let lo = hi.saturating_sub(1);
        let a = &self.clips[lo];
        let b = &self.clips[hi];
        let weight = if lo == hi {
            0.
        } else {
            ((self.axis - a.0) / (b.0 - a.0)).clamp(0., 1.)
        };
        Ok((clip(model, &a.1)?, clip(model, &b.1)?, weight))
    }
}
// State machine: ordered, first matching edge; frozen outgoing local pose during a fade.
#[derive(Default, Clone, Debug, Data)]
pub enum Param {
    #[default]
    Unset,
    Number(f32),
    Flag(bool),
}
impl From<f32> for Param {
    fn from(v: f32) -> Self {
        assert!(v.is_finite());
        Self::Number(v)
    }
}
impl From<bool> for Param {
    fn from(v: bool) -> Self {
        Self::Flag(v)
    }
}
#[derive(Default, Clone, Copy, Debug, Data)]
pub enum Cmp {
    Lt,
    Le,
    #[default]
    Eq,
    Ge,
    Gt,
}
#[derive(Clone, Debug, Data)]
pub enum Condition {
    Arg(String, Cmp, Param),
}
impl Default for Condition {
    fn default() -> Self {
        Self::Arg(String::new(), Cmp::Eq, Param::Unset)
    }
}
impl Condition {
    fn matches(&self, params: &[(String, Param)]) -> bool {
        let Self::Arg(name, cmp, value) = self;
        let Some((_, v)) = params.iter().find(|p| &p.0 == name) else {
            return false;
        };
        let order = match (v, value) {
            (Param::Number(a), Param::Number(b)) => a.partial_cmp(b),
            (Param::Flag(a), Param::Flag(b)) => a.partial_cmp(b),
            _ => None,
        };
        order.is_some_and(|o| match cmp {
            Cmp::Lt => o.is_lt(),
            Cmp::Le => !o.is_gt(),
            Cmp::Eq => o.is_eq(),
            Cmp::Ge => !o.is_lt(),
            Cmp::Gt => o.is_gt(),
        })
    }
}
#[derive(Clone, Debug, Data)]
pub enum Play {
    Clip(String),
    Blend(Blend),
}
impl Default for Play {
    fn default() -> Self {
        Self::Clip(String::new())
    }
}
#[derive(Default, Clone, Debug, Data)]
pub struct State {
    pub name: String,
    pub play: Play,
    pub transitions: Vec<(String, Condition)>,
    /// Seconds to fade into this state.
    pub fade: f32,
}
impl State {
    pub fn new(name: impl Into<String>, play: Play) -> Self {
        Self {
            name: name.into(),
            play,
            ..Self::default()
        }
    }
    pub fn to(mut self, to: impl Into<String>, when: Condition) -> Self {
        self.transitions.push((to.into(), when));
        self
    }
    pub fn fade(mut self, seconds: f32) -> Self {
        assert!(seconds.is_finite() && seconds >= 0.);
        self.fade = seconds;
        self
    }
}
#[derive(Default, Clone, Debug, Component)]
pub struct Animator {
    pub states: Vec<State>,
    pub current: u32,
    pub since: f32,
    pub params: Vec<(String, Param)>,
    from: Vec<f32>,
    fade_time: f32,
    fade_duration: f32,
}
impl Animator {
    pub fn new(states: impl IntoIterator<Item = State>) -> Self {
        Self {
            states: states.into_iter().collect(),
            ..Self::default()
        }
    }
    pub fn set(&mut self, name: &str, value: impl Into<Param>) {
        let value = value.into();
        if let Some(p) = self.params.iter_mut().find(|p| p.0 == name) {
            p.1 = value;
        } else {
            self.params.push((name.into(), value));
        }
    }
    pub fn state(&self) -> &str {
        self.states
            .get(self.current as usize)
            .map_or("", |s| s.name.as_str())
    }
    fn advance(
        &mut self,
        pose: &mut Pose,
        model: &Model,
        dt: f32,
        scratch: &mut Vec<f32>,
        rest: &[f32],
    ) -> Result<(), String> {
        let state = self
            .states
            .get(self.current as usize)
            .ok_or("animator current state out of range")?;
        if let Some((to, _)) = state
            .transitions
            .iter()
            .find(|(_, c)| c.matches(&self.params))
        {
            let next = self
                .states
                .iter()
                .position(|s| &s.name == to)
                .ok_or_else(|| format!("unknown state `{to}`"))?;
            self.from.clone_from(&pose.local);
            self.current = next as u32;
            self.since = 0.;
            self.fade_time = 0.;
            self.fade_duration = self.states[next].fade;
            if !self.fade_duration.is_finite() || self.fade_duration < 0. {
                return Err("invalid fade duration".into());
            }
            // Keep normalized phase through locomotion state transitions.
        }
        self.since += dt;
        let play = &self.states[self.current as usize].play;
        let pair = match play {
            Play::Clip(n) => {
                let c = clip(model, n)?;
                (c, c, 0.)
            }
            Play::Blend(b) => b.pair(model)?,
        };
        advance_pair(pair, pose, dt, scratch, rest);
        if self.fade_time < self.fade_duration && self.from.len() == pose.local.len() {
            self.fade_time = (self.fade_time + dt).min(self.fade_duration);
            mix_pose(
                &self.from,
                &mut pose.local,
                self.fade_time / self.fade_duration,
            );
        }
        Ok(())
    }
}
/// Compact saved arrays, ten floats per imported node: translation, xyzw rotation, scale.
/// Imported nodes are never entities. The composed palette is not saved.
#[derive(Default, Clone, Debug, Component)]
pub struct Pose {
    pub previous: Vec<f32>,
    pub local: Vec<f32>,
    pub phase: f32,
    pub root_motion: Vec3,
    pub crossed: Vec<String>,
    pub bounds: [f32; 6],
    stepped: Option<u64>,
}
#[derive(Default, Clone, Debug, Component)]
pub struct Ik {
    pub chain: [String; 3],
    pub target: Vec3,
    pub pole: Vec3,
    pub weight: f32,
}
/// Declare one gameplay socket on the owning model. Attachments never address bone entities.
#[derive(Default, Clone, Debug, Component)]
pub struct Socket(pub String);
#[derive(Default, Clone, Debug, Component)]
pub struct SocketPose(pub Transform);
#[derive(Default, Clone, Debug, Component)]
pub struct SocketFollow {
    pub target: crate::FollowTarget,
    pub offset: Transform,
}
impl SocketFollow {
    pub fn new(target: impl Into<crate::FollowTarget>) -> Self {
        Self {
            target: target.into(),
            ..Self::default()
        }
    }
}
#[derive(Default)]
pub(crate) struct Runtime {
    entities: Vec<Entity>,
    rigs: BTreeMap<String, Rig>,
    scratch: Vec<f32>,
}
struct Rig {
    rest: Vec<f32>,
    bounds: [f32; 6],
}
pub(crate) fn register<C: Component>(w: &mut World) {
    if [
        TypeId::of::<Animation>(),
        TypeId::of::<Blend>(),
        TypeId::of::<Animator>(),
        TypeId::of::<Socket>(),
    ]
    .contains(&TypeId::of::<C>())
    {
        w.animation_tick = Some(step);
        w.register::<Pose>().register::<SocketPose>();
    }
}
impl Rig {
    fn new(model: &Model) -> Self {
        Self {
            rest: bind_pose(model),
            bounds: animated_bounds(model),
        }
    }
}
pub(crate) fn conflicts<C: Component>(w: &World, e: Entity) -> bool {
    let id = TypeId::of::<C>();
    [
        TypeId::of::<Animation>(),
        TypeId::of::<Blend>(),
        TypeId::of::<Animator>(),
    ]
    .contains(&id)
        && ((id != TypeId::of::<Animation>() && w.has::<Animation>(e))
            || (id != TypeId::of::<Blend>() && w.has::<Blend>(e))
            || (id != TypeId::of::<Animator>() && w.has::<Animator>(e)))
}
fn clip<'a>(model: &'a Model, name: &str) -> Result<&'a Clip, String> {
    model
        .clips
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| format!("unknown clip `{name}`"))
}
impl Clip {
    pub fn duration(&self) -> f32 {
        self.tracks
            .iter()
            .filter_map(|t| t.times.last())
            .copied()
            .fold(0., f32::max)
    }
}
fn wrap(time: f32, duration: f32) -> f32 {
    if duration > 0. {
        time - math::floor(time / duration) * duration
    } else {
        0.
    }
}
fn at(p: &[f32]) -> Transform {
    Transform {
        position: Vec3::from_slice(p),
        rotation: Quat::from_xyzw(p[3], p[4], p[5], p[6]),
        scale: Vec3::from_slice(&p[7..]),
    }
}
fn put(p: &mut [f32], t: Transform) {
    p[..3].copy_from_slice(&t.position.to_array());
    p[3..7].copy_from_slice(&t.rotation.to_array());
    p[7..10].copy_from_slice(&t.scale.to_array());
}
fn matrix(p: &[f32]) -> Mat4 {
    let t = at(p);
    Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
}
pub fn bind_pose(model: &Model) -> Vec<f32> {
    let mut out = vec![0.; model.nodes.len() * 10];
    for (n, p) in model.nodes.iter().zip(out.chunks_exact_mut(10)) {
        let (scale, rotation, position) =
            Mat4::from_cols_array(&n.transform).to_scale_rotation_translation();
        put(
            p,
            Transform {
                position,
                rotation: rotation.normalize(),
                scale,
            },
        );
    }
    out
}
/// Parent-first traversal used by the GPU loader; joint indices keep glTF's order.
pub fn node_order(model: &Model) -> Vec<u32> {
    let mut order = Vec::with_capacity(model.nodes.len());
    while order.len() < model.nodes.len() {
        let before = order.len();
        for (i, n) in model.nodes.iter().enumerate() {
            if !order.contains(&(i as u32)) && n.parent.is_none_or(|p| order.contains(&p)) {
                order.push(i as u32);
            }
        }
        assert!(order.len() > before, "validated parent graph");
    }
    order
}
fn mix_pose(from: &[f32], to: &mut [f32], weight: f32) {
    for (a, b) in from.chunks_exact(10).zip(to.chunks_exact_mut(10)) {
        let a = at(a);
        let t = at(b);
        put(
            b,
            Transform {
                position: a.position.lerp(t.position, weight),
                rotation: a.rotation.slerp(t.rotation, weight).normalize(),
                scale: a.scale.lerp(t.scale, weight),
            },
        );
    }
}
fn value(track: &Track, time: f32) -> [f32; 4] {
    let arity = if matches!(track.path, TrackPath::Rotation) {
        4
    } else {
        3
    };
    let cubic = matches!(track.interpolation, Interpolation::CubicSpline);
    let stride = arity * if cubic { 3 } else { 1 };
    let index = track
        .times
        .partition_point(|v| *v <= time)
        .saturating_sub(1);
    let next = (index + 1).min(track.times.len() - 1);
    let a = index * stride + if cubic { arity } else { 0 };
    let b = next * stride + if cubic { arity } else { 0 };
    let mut out = [0.; 4];
    out[..arity].copy_from_slice(&track.values[a..a + arity]);
    if index == next || time <= track.times[0] || matches!(track.interpolation, Interpolation::Step)
    {
        return out;
    }
    let span = track.times[next] - track.times[index];
    let t = (time - track.times[index]) / span;
    if matches!(track.path, TrackPath::Rotation) && !cubic {
        return Quat::from_array(out)
            .normalize()
            .slerp(Quat::from_slice(&track.values[b..b + 4]).normalize(), t)
            .normalize()
            .to_array();
    }
    for (j, v) in out.iter_mut().enumerate().take(arity) {
        *v = if cubic {
            let t2 = t * t;
            let t3 = t2 * t;
            (2. * t3 - 3. * t2 + 1.) * track.values[a + j]
                + (t3 - 2. * t2 + t) * span * track.values[a + arity + j]
                + (-2. * t3 + 3. * t2) * track.values[b + j]
                + (t3 - t2) * span * track.values[b - arity + j]
        } else {
            math::lerp(*v, track.values[b + j], t)
        };
    }
    if matches!(track.path, TrackPath::Rotation) {
        out = Quat::from_array(out).normalize().to_array();
    }
    out
}
pub fn sample(clip: &Clip, time: f32, rest: &[f32], out: &mut Vec<f32>) {
    out.clear();
    out.extend_from_slice(rest);
    for track in &clip.tracks {
        let v = value(track, time);
        let p = &mut out[track.node as usize * 10..][..10];
        match track.path {
            TrackPath::Translation => p[..3].copy_from_slice(&v[..3]),
            TrackPath::Rotation => {
                p[3..7].copy_from_slice(&Quat::from_array(v).normalize().to_array())
            }
            TrackPath::Scale => p[7..10].copy_from_slice(&v[..3]),
        }
    }
}
fn crossed(mark: f32, old: f32, new: f32, duration: f32, looping: bool) -> bool {
    if old == new || mark < 0. || mark > duration {
        return false;
    }
    if looping && duration > 0. {
        math::floor((old - mark) / duration) != math::floor((new - mark) / duration)
    } else if new > old {
        old < mark && new >= mark
    } else {
        new <= mark && old > mark
    }
}
fn markers(
    clip: &Clip,
    extra: &[(f32, String)],
    old: f32,
    new: f32,
    looping: bool,
    out: &mut Vec<String>,
) {
    for (t, name) in clip.markers.iter().chain(extra) {
        if crossed(*t, old, new, clip.duration(), looping) && !out.contains(name) {
            out.push(name.clone());
        }
    }
}
fn advance_pair(
    (a, b, weight): (&Clip, &Clip, f32),
    p: &mut Pose,
    dt: f32,
    scratch: &mut Vec<f32>,
    rest: &[f32],
) {
    let duration = math::lerp(a.duration(), b.duration(), weight);
    let old = p.phase;
    let next = old + if duration > 0. { dt / duration } else { 0. };
    p.phase = wrap(next, 1.);
    sample(a, p.phase * a.duration(), rest, &mut p.local);
    if weight > 0. {
        sample(b, p.phase * b.duration(), rest, scratch);
        mix_pose(&p.local, scratch, weight);
        p.local.copy_from_slice(scratch);
    }
    markers(
        a,
        &[],
        old * a.duration(),
        next * a.duration(),
        true,
        &mut p.crossed,
    );
    if weight > 0. {
        markers(
            b,
            &[],
            old * b.duration(),
            next * b.duration(),
            true,
            &mut p.crossed,
        );
    }
}
/// Deterministic model-local joint matrix, composing only the requested ancestor chain.
pub fn joint_matrix(model: &Model, local: &[f32], node: u32) -> Mat4 {
    let mut out = matrix(&local[node as usize * 10..]);
    let mut parent = model.nodes[node as usize].parent;
    while let Some(p) = parent {
        out = matrix(&local[p as usize * 10..]) * out;
        parent = model.nodes[p as usize].parent;
    }
    out
}
/// Analytic two-bone IK in model space. Pole selects the bend plane; weight zero is exact.
pub fn solve_ik(model: &Model, local: &mut [f32], ik: &Ik) -> Result<(), String> {
    if ik.weight == 0. {
        return Ok(());
    }
    if !ik.target.is_finite() || !ik.pole.is_finite() || !ik.weight.is_finite() {
        return Err("non-finite IK".into());
    }
    let mut ids = [0u32; 3];
    for (i, name) in ik.chain.iter().enumerate() {
        ids[i] = model
            .nodes
            .iter()
            .position(|n| &n.name == name)
            .ok_or_else(|| format!("unknown IK joint `{name}`"))? as u32;
    }
    if model.nodes[ids[1] as usize].parent != Some(ids[0])
        || model.nodes[ids[2] as usize].parent != Some(ids[1])
    {
        return Err("IK requires a direct root/mid/tip chain".into());
    }
    let globals = ids.map(|i| joint_matrix(model, local, i));
    let [a, b, c] = globals.map(|m| m.w_axis.truncate());
    let l1 = a.distance(b);
    let l2 = b.distance(c);
    let delta = ik.target - a;
    let distance = delta.length();
    if l1 < 1e-6 || l2 < 1e-6 || distance < 1e-6 {
        return Err("degenerate IK chain/target".into());
    }
    let direction = delta / distance;
    let d = distance.clamp((l1 - l2).abs().max(1e-6), l1 + l2);
    let pole = ik.pole - a;
    let mut bend = (pole - direction * pole.dot(direction)).normalize_or_zero();
    if bend == Vec3::ZERO {
        bend = direction.any_orthonormal_vector();
    }
    let x = ((l1 * l1 - l2 * l2 + d * d) / (2. * d)).clamp(-l1, l1);
    let y = math::sqrt((l1 * l1 - x * x).max(0.));
    let mid = a + direction * x + bend * y;
    let tip = a + direction * d;
    let weight = ik.weight.clamp(0., 1.);
    let root = ids[0] as usize * 10;
    let old_root = at(&local[root..]).rotation;
    let parent = model.nodes[ids[0] as usize]
        .parent
        .map_or(Quat::IDENTITY, |i| {
            joint_matrix(model, local, i)
                .to_scale_rotation_translation()
                .1
        });
    let rotation = parent.inverse()
        * Quat::from_rotation_arc((b - a) / l1, (mid - a).normalize())
        * globals[0].to_scale_rotation_translation().1;
    local[root + 3..root + 7].copy_from_slice(&rotation.normalize().to_array());
    let m = joint_matrix(model, local, ids[1]);
    let end = joint_matrix(model, local, ids[2]).w_axis.truncate();
    let start = m.w_axis.truncate();
    let mid_index = ids[1] as usize * 10;
    let old_mid = at(&local[mid_index..]).rotation;
    let parent = joint_matrix(model, local, ids[0])
        .to_scale_rotation_translation()
        .1;
    let rotation = parent.inverse()
        * Quat::from_rotation_arc((end - start).normalize(), (tip - start).normalize())
        * m.to_scale_rotation_translation().1;
    local[mid_index + 3..mid_index + 7].copy_from_slice(
        &old_mid
            .slerp(rotation.normalize(), weight)
            .normalize()
            .to_array(),
    );
    let solved = at(&local[root..]).rotation;
    local[root + 3..root + 7]
        .copy_from_slice(&old_root.slerp(solved, weight).normalize().to_array());
    Ok(())
}
// Conservative reach: maximum sum of local translation lengths along a chain,
// scaled by ancestor scale, plus each influenced vertex's inverse-bind radius.
pub fn animated_bounds(model: &Model) -> [f32; 6] {
    if model.skins.is_empty() {
        return model.bounds;
    }
    let rest = bind_pose(model);
    let mut lengths = vec![0.; model.nodes.len()];
    let mut scales = vec![1.; model.nodes.len()];
    for (i, p) in rest.chunks_exact(10).enumerate() {
        lengths[i] = Vec3::from_slice(p).length();
        scales[i] = Vec3::from_slice(&p[7..]).abs().max_element();
    }
    for clip in &model.clips {
        for t in &clip.tracks {
            if matches!(t.path, TrackPath::Translation | TrackPath::Scale) {
                // Cubic tangents can overshoot; include one duration times both tangents.
                let bound = t
                    .values
                    .chunks_exact(3)
                    .map(|p| Vec3::from_slice(p).length())
                    .fold(0., f32::max)
                    * if matches!(t.interpolation, Interpolation::CubicSpline) {
                        1. + 2. * clip.duration()
                    } else {
                        1.
                    };
                let slot = if matches!(t.path, TrackPath::Translation) {
                    &mut lengths[t.node as usize]
                } else {
                    &mut scales[t.node as usize]
                };
                *slot = slot.max(bound);
            }
        }
    }
    let mut reach = vec![0.; model.nodes.len()];
    let mut global_scale = vec![1.; model.nodes.len()];
    for i in node_order(model) {
        let i = i as usize;
        let (r, s) = model.nodes[i]
            .parent
            .map_or((0., 1.), |p| (reach[p as usize], global_scale[p as usize]));
        reach[i] = r + lengths[i] * s;
        global_scale[i] = s * scales[i];
    }
    let mut radius = reach.iter().copied().fold(0., f32::max);
    for node in &model.nodes {
        if let (Some(mesh), Some(skin)) = (node.mesh, node.skin) {
            let mesh = &model.meshes[mesh as usize];
            let skin = &model.skins[skin as usize];
            for (v, position) in mesh.positions.chunks_exact(3).enumerate() {
                for influence in 0..4 {
                    let j = mesh.joints[v * 4 + influence] as usize;
                    let node = skin.joints[j] as usize;
                    let inverse = Mat4::from_cols_slice(&skin.inverse_binds[j * 16..j * 16 + 16]);
                    radius = radius.max(
                        reach[node]
                            + global_scale[node]
                                * inverse
                                    .transform_point3(Vec3::from_slice(position))
                                    .length(),
                    );
                }
            }
        }
    }
    let b = model.bounds;
    [
        b[0] - radius,
        b[1] - radius,
        b[2] - radius,
        b[3] + radius,
        b[4] + radius,
        b[5] + radius,
    ]
}
/// Evaluate clips after game-authored parameters. Called automatically once per fixed tick.
pub fn step(w: &mut World) {
    if w.storage::<Animation>().is_none_or(|s| s.is_empty())
        && w.storage::<Blend>().is_none_or(|s| s.is_empty())
        && w.storage::<Animator>().is_none_or(|s| s.is_empty())
        && w.storage::<Socket>().is_none_or(|s| s.is_empty())
    {
        return;
    }
    let mut runtime = std::mem::take(&mut w.animation);
    runtime.entities.clear();
    runtime
        .entities
        .extend(w.query::<&Animation>().iter().map(|(e, _)| e));
    runtime
        .entities
        .extend(w.query::<&Blend>().iter().map(|(e, _)| e));
    runtime
        .entities
        .extend(w.query::<&Animator>().iter().map(|(e, _)| e));
    runtime
        .entities
        .extend(w.query::<&Socket>().iter().map(|(e, _)| e));
    runtime.entities.sort_unstable();
    runtime.entities.dedup();
    for &e in &runtime.entities {
        if w.get::<Pose>(e)
            .is_some_and(|p| p.stepped == Some(w.tick()))
        {
            continue;
        }
        let result = (|| {
            let mesh = w.get::<Mesh>(e).ok_or("animation needs a mesh")?;
            let Mesh::Asset(name) = &*mesh else {
                return Err("animation needs Mesh::asset".into());
            };
            if !w.assets.declared.contains(name) {
                return Err(format!("animation model `{name}` must be in Game::ASSETS"));
            }
            let model = w
                .assets
                .models
                .get(name)
                .cloned()
                .ok_or("animation model not loaded")?;
            if model.nodes.len() > 256 {
                return Err("animation supports at most 256 imported nodes".into());
            }
            let rig = runtime
                .rigs
                .entry(name.clone())
                .or_insert_with(|| Rig::new(&model));
            drop(mesh);
            if !w.has::<Pose>(e) {
                w.insert(
                    e,
                    Pose {
                        previous: rig.rest.clone(),
                        local: rig.rest.clone(),
                        bounds: rig.bounds,
                        ..Pose::default()
                    },
                );
            }
            let mut pose = w.get_mut::<Pose>(e).unwrap();
            if pose.local.len() != rig.rest.len() || pose.previous.len() != rig.rest.len() {
                return Err("saved pose does not match model".into());
            }
            let p = &mut *pose;
            p.previous.copy_from_slice(&p.local);
            p.crossed.clear();
            p.root_motion = Vec3::ZERO;
            if let Some(mut a) = w.get_mut::<Animation>(e) {
                if !a.time.is_finite() || !a.speed.is_finite() {
                    return Err("non-finite animation clock".into());
                }
                let c = clip(&model, &a.clip)?;
                let duration = c.duration();
                let old = a.time;
                let next = old + w.dt() * a.speed;
                a.time = if a.looping {
                    wrap(next, duration)
                } else {
                    next.clamp(0., duration)
                };
                sample(c, a.time, &rig.rest, &mut p.local);
                markers(
                    c,
                    &a.markers,
                    old,
                    if a.looping { next } else { a.time },
                    a.looping,
                    &mut p.crossed,
                );
                a.crossed.clone_from(&p.crossed);
                if let Some(root) = model.skins.first().and_then(|s| s.joints.first()) {
                    for track in &c.tracks {
                        if track.node == *root && matches!(track.path, TrackPath::Translation) {
                            p.root_motion = Vec3::from_slice(&value(track, a.time))
                                - Vec3::from_slice(&value(track, old));
                            if a.looping && duration > 0. {
                                p.root_motion += (Vec3::from_slice(&value(track, duration))
                                    - Vec3::from_slice(&value(track, 0.)))
                                    * math::floor(next / duration);
                            }
                        }
                    }
                }
                a.root_motion = p.root_motion;
            } else if let Some(b) = w.get::<Blend>(e) {
                advance_pair(b.pair(&model)?, p, w.dt(), &mut runtime.scratch, &rig.rest);
            } else if let Some(mut a) = w.get_mut::<Animator>(e) {
                a.advance(p, &model, w.dt(), &mut runtime.scratch, &rig.rest)?;
            } else {
                p.local.copy_from_slice(&rig.rest);
            }
            if let Some(ik) = w.get::<Ik>(e) {
                solve_ik(&model, &mut p.local, &ik)?;
            }
            p.stepped = Some(w.tick());
            for marker in &p.crossed {
                let name = w.name(e).unwrap_or("unnamed");
                let playing = w
                    .get::<Animation>(e)
                    .map(|a| a.clip.clone())
                    .unwrap_or_else(|| {
                        w.get::<Animator>(e)
                            .map_or_else(|| "blend".into(), |a| a.state().into())
                    });
                w.log(format_args!("animation {name} {playing} {marker}"));
            }
            let socket = w.get::<Socket>(e).map(|s| s.0.clone());
            let socket = socket
                .map(|name| {
                    let i = model
                        .nodes
                        .iter()
                        .position(|n| n.name == name)
                        .ok_or_else(|| format!("unknown socket `{name}`"))?;
                    let (scale, rotation, position) =
                        joint_matrix(&model, &p.local, i as u32).to_scale_rotation_translation();
                    Ok::<_, String>(SocketPose(Transform {
                        position,
                        rotation,
                        scale,
                    }))
                })
                .transpose()?;
            drop(pose);
            if let Some(socket) = socket {
                w.insert(e, socket);
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            w.log(format_args!("animation #{}: {error}", e.index()));
        }
    }
    for (e, follow) in w.query::<&SocketFollow>().iter() {
        let target = match &follow.target {
            crate::FollowTarget::Entity(e) => Some(*e),
            crate::FollowTarget::Name(n) => w.named(n),
        };
        let Some(target) = target else { continue };
        let Some(socket) = w.get::<SocketPose>(target) else {
            continue;
        };
        let affine = |t: Transform| {
            crate::Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position)
        };
        let Some(global) = w.current_global(target) else {
            continue;
        };
        let mut result = global * affine(socket.0) * affine(follow.offset);
        if let Some(parent) = w
            .get::<crate::Parent>(e)
            .and_then(|p| w.current_global(p.0))
        {
            result = parent.inverse() * result;
        }
        if let Some(mut transform) = w.get_mut::<Transform>(e) {
            let (scale, rotation, position) = result.to_scale_rotation_translation();
            *transform = Transform {
                position,
                rotation,
                scale,
            };
        }
    }
    w.animation = runtime;
    w.propagate();
}
/// Agent inspection only: world-space matrices for the skin's joints, capped at 256.
pub fn pose_json(w: &World, e: Entity) -> Result<String, String> {
    let mesh = w.get::<Mesh>(e).ok_or("pose needs a model")?;
    let Mesh::Asset(name) = &*mesh else {
        return Err("pose needs a model".into());
    };
    let model = w.model(name).ok_or("pose needs a declared model")?;
    let bind;
    let pose = w.get::<Pose>(e);
    let local = if let Some(p) = &pose {
        &p.local
    } else {
        bind = bind_pose(model);
        &bind
    };
    let global = Mat4::from(w.current_global(e).unwrap_or(crate::Affine3A::IDENTITY));
    let mut rows = Vec::new();
    for &i in model.skins.iter().flat_map(|s| &s.joints).take(256) {
        rows.push(format!(
            "{{\"name\":{},\"world\":{}}}",
            crate::values::quote(&model.nodes[i as usize].name),
            crate::json::to_string(&(global * joint_matrix(model, local, i)).to_cols_array())
                .map_err(|e| e.to_string())?
        ));
    }
    Ok(format!("[{}]", rows.join(",")))
}

/// Fresh animation declarations retained briefly by the model adapter during dev carry.
/// Open restores exactly; Carry overlays definitions while retaining the sampled situation.
#[derive(Default)]
pub struct Definitions(Vec<Declaration>);
type Declaration = (String, Option<Animation>, Option<Blend>, Option<Animator>);
impl Definitions {
    pub fn capture(w: &World) -> Self {
        Self(
            w.entities()
                .filter_map(|e| {
                    let a = w.get::<Animation>(e).map(|v| v.clone());
                    let b = w.get::<Blend>(e).map(|v| v.clone());
                    let c = w.get::<Animator>(e).map(|v| v.clone());
                    (a.is_some() || b.is_some() || c.is_some())
                        .then(|| (w.name(e).unwrap_or("").into(), a, b, c))
                })
                .collect(),
        )
    }
    pub fn apply(self, w: &World) {
        for (name, a, b, c) in self.0 {
            let Some(e) = w.named(&name) else { continue };
            if let (Some(fresh), Some(mut old)) = (a, w.get_mut::<Animation>(e)) {
                old.clip = fresh.clip;
                old.speed = fresh.speed;
                old.looping = fresh.looping;
                old.markers = fresh.markers;
            }
            if let (Some(fresh), Some(mut old)) = (b, w.get_mut::<Blend>(e)) {
                old.clips = fresh.clips;
            }
            if let (Some(fresh), Some(mut old)) = (c, w.get_mut::<Animator>(e)) {
                let name = old.state();
                if let Some(current) = fresh.states.iter().position(|s| s.name == name) {
                    if crate::hash::of(&old.states) != crate::hash::of(&fresh.states) {
                        // An edited blend starts from the carried pose, never from bind.
                        if let Some(pose) = w.get::<Pose>(e) {
                            old.from.clone_from(&pose.local);
                            old.fade_time = 0.;
                            old.fade_duration = fresh.states[current].fade.max(0.1);
                        }
                        old.states = fresh.states;
                        old.current = current as u32;
                    }
                }
            }
        }
    }
}

pub(crate) fn status_json(w: &World, e: Entity) -> Result<String, String> {
    if let Some(a) = w.get::<Animation>(e) {
        return crate::json::to_string(&*a)
            .map(|s| format!(",\"animation\":{s}"))
            .map_err(|e| e.to_string());
    }
    if let Some(b) = w.get::<Blend>(e) {
        return crate::json::to_string(&*b)
            .map(|s| format!(",\"blend\":{s}"))
            .map_err(|e| e.to_string());
    }
    if let Some(a) = w.get::<Animator>(e) {
        return Ok(format!(
            ",\"animator\":{{\"state\":{},\"since\":{},\"params\":{}}}",
            crate::values::quote(a.state()),
            crate::json::to_string(&a.since).map_err(|e| e.to_string())?,
            crate::json::to_string(&a.params).map_err(|e| e.to_string())?
        ));
    }
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{Node, Skin};
    fn translation(name: &str, seconds: f32, distance: f32) -> Clip {
        Clip {
            name: name.into(),
            tracks: vec![Track {
                times: vec![0., seconds],
                values: vec![0., 0., 0., distance, 0., 0.],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    fn world() -> World {
        let mut w = World::new(60, 0);
        w.register_scene();
        w.assets.declared.insert("rig.model".into());
        w.assets.models.insert(
            "rig.model".into(),
            std::sync::Arc::new(Model {
                nodes: vec![Node::default()],
                skins: vec![Skin {
                    joints: vec![0],
                    inverse_binds: Mat4::IDENTITY.to_cols_array().to_vec(),
                    ..Default::default()
                }],
                clips: vec![translation("slow", 1., 1.), translation("fast", 0.5, 1.)],
                ..Default::default()
            }),
        );
        w
    }
    #[test]
    fn clock_rounding_markers_loop_and_root_contribution_are_saved() {
        let mut w = world();
        let e = w.spawn_named(
            "actor",
            (
                Transform::default(),
                Mesh::asset("rig.model"),
                Animation::play("slow").marker(0.3, "step"),
            ),
        );
        let mut events = 0;
        for _ in 0..60 {
            w.step_clock();
            events += u32::from(w.get::<Animation>(e).unwrap().crossed("step"));
        }
        assert_eq!(w.get::<Animation>(e).unwrap().time.to_bits(), 0x3f7ffffb);
        assert_eq!(events, 1);
        assert_eq!(w.get::<Transform>(e).unwrap().position, Vec3::ZERO);
        assert_eq!(
            w.journal()
                .iter()
                .filter(|e| e.line.ends_with(" animation actor slow step"))
                .count(),
            1
        );
        let before = w.save();
        let hash = w.hash();
        w.load(&before).unwrap();
        assert_eq!(hash, w.hash());
        w.step_clock();
        assert!((w.get::<Animation>(e).unwrap().root_motion().x - 1. / 60.).abs() < 1e-6);
        assert!(!w.insert(e, Blend::across([(0., "slow")])));
    }
    #[test]
    fn blend_uses_normalized_phase_and_once_stops() {
        let mut w = world();
        let mut blend = Blend::across([(0., "slow"), (1., "fast")]);
        blend.axis = 0.5;
        let e = w.spawn((Mesh::asset("rig.model"), blend));
        w.step_clock();
        let p = w.get::<Pose>(e).unwrap();
        assert!((p.phase - (1. / 60.) / 0.75).abs() < 1e-7);
        assert!((p.local[0] - p.phase).abs() < 1e-7);
        drop(p);
        let a = w.spawn((
            Mesh::asset("rig.model"),
            Animation::play("fast").once().marker(0.5, "end"),
        ));
        for _ in 0..30 {
            w.step_clock();
        }
        assert!(w.get::<Animation>(a).unwrap().crossed("end"));
        w.step_clock();
        let a = w.get::<Animation>(a).unwrap();
        assert_eq!(a.time, 0.5);
        assert!(!a.crossed("end"));
    }
    #[test]
    fn step_linear_cubic_rotation_and_parent_order() {
        let mut track = translation("test", 2., 2.).tracks.remove(0);
        assert_eq!(value(&track, 1.)[0], 1.);
        track.interpolation = Interpolation::Step;
        assert_eq!(value(&track, 1.)[0], 0.);
        track.interpolation = Interpolation::CubicSpline;
        track.values = vec![
            0., 0., 0., 0., 0., 0., 1., 0., 0., 1., 0., 0., 2., 0., 0., 0., 0., 0.,
        ];
        assert_eq!(value(&track, 1.)[0], 1.);
        let q = Quat::from_rotation_y(1.);
        track.path = TrackPath::Rotation;
        track.interpolation = Interpolation::Linear;
        track.values = [Quat::IDENTITY.to_array(), q.to_array()].concat();
        let got = Quat::from_array(value(&track, 1.));
        assert!(got.dot(Quat::from_rotation_y(0.5)) > 0.999999);
        let m = Model {
            nodes: vec![
                Node {
                    parent: Some(1),
                    ..Default::default()
                },
                Node::default(),
            ],
            ..Default::default()
        };
        assert_eq!(node_order(&m), [1, 0]);
    }
}
