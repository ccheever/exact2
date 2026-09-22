//! Saved clip clocks and local poses. Games call `step` explicitly before reading
//! markers or root motion; a second call in one tick never advances playback.
#![allow(missing_docs)]
pub use crate::asset::pose::{animated_bounds, bind_pose, joint_matrix, node_order, Pose};
use crate::asset::pose::{at, put};
use crate::{
    asset::{Clip, Interpolation, Model, Track, TrackPath},
    math, Component, Data, Entity, Mesh, Quat, Transform, Vec3, World,
};
use glam::Mat4;
mod sockets;
use sockets::SocketCache;
pub use sockets::{socket, socket_matrix, socket_node, socket_stale, Motion, SocketFollow};
use std::{any::TypeId, collections::BTreeMap, sync::Arc};

/// Saved output shared by every playback controller. Declare the motion root by node name.
#[derive(Default, Clone, Debug, Data)]
pub struct Playback {
    pub motion_root: Option<String>,
    crossed: Vec<String>,
    root_motion: Vec3,
}
impl Playback {
    pub fn crossed(&self, name: &str) -> bool {
        self.crossed.iter().any(|n| n == name)
    }
    /// Model-local translation extracted this tick; apply through the entity's scale/rotation.
    pub fn root_motion(&self) -> Vec3 {
        self.root_motion
    }
    fn clear(&mut self) {
        self.crossed.clear();
        self.root_motion = Vec3::ZERO;
    }
    fn record(&mut self, pose: &Pose) {
        self.crossed.clone_from(&pose.crossed);
        self.root_motion = pose.root_motion;
    }
    fn root(&self, model: &Model) -> Result<Option<u32>, String> {
        self.motion_root
            .as_ref()
            .map(|name| {
                named_node(model, name).ok_or_else(|| format!("unknown motion root `{name}`"))
            })
            .transpose()
    }
}
macro_rules! playback {
    ($($ty:ty),*) => {$ (
        impl std::ops::Deref for $ty {
            type Target = Playback;
            fn deref(&self) -> &Playback { &self.playback }
        }
        impl $ty {
            pub fn motion_root(mut self, name: impl Into<String>) -> Self {
                self.playback.motion_root = Some(name.into()); self
            }
        }
    )*};
}
playback!(Animation, Blend, Animator);

#[derive(Clone, Debug, Data)]
pub struct Animation {
    pub clip: String,
    pub time: f32,
    /// Whether this controller has successfully sampled; saved across restore.
    pub sampled: bool,
    pub speed: f32,
    pub looping: bool,
    pub markers: Vec<(f32, String)>,
    pub playback: Playback,
}
impl Default for Animation {
    fn default() -> Self {
        Self {
            clip: String::new(),
            time: 0.,
            sampled: false,
            speed: 1.,
            looping: true,
            markers: vec![],
            playback: Playback::default(),
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
}
#[derive(Default, Clone, Debug, Data)]
pub struct Blend {
    pub playback: Playback,
    pub parameter: String,
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
            ..Self::default()
        }
    }
    /// Bind this axis to an Animator number parameter; standalone blends use `axis`.
    pub fn parameter(mut self, name: impl Into<String>) -> Self {
        self.parameter = name.into();
        self
    }
    fn pair<'a>(
        &self,
        model: &'a Model,
        params: &[(String, Param)],
    ) -> Result<(&'a Clip, &'a Clip, f32), String> {
        let axis = if self.parameter.is_empty() {
            self.axis
        } else {
            match params.iter().find(|p| p.0 == self.parameter) {
                Some((_, Param::Number(v))) => *v,
                None => self.axis,
                _ => {
                    return Err(format!(
                        "blend parameter `{}` must be a number",
                        self.parameter
                    ))
                }
            }
        };
        if !axis.is_finite()
            || self.clips.is_empty()
            || self.clips.iter().any(|c| !c.0.is_finite())
            || self.clips.windows(2).any(|p| p[0].0 >= p[1].0)
        {
            return Err("invalid blend axis/knots".into());
        }
        let hi = self
            .clips
            .partition_point(|p| p.0 < axis)
            .min(self.clips.len() - 1);
        if hi == 0 || axis >= self.clips[hi].0 {
            let c = clip(model, &self.clips[hi].1)?;
            return Ok((c, c, 0.));
        }
        let lo = hi - 1;
        let a = &self.clips[lo];
        let b = &self.clips[hi];
        let weight = if lo == hi {
            0.
        } else {
            ((axis - a.0) / (b.0 - a.0)).clamp(0., 1.)
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
    pub fn gt(name: impl Into<String>, value: f32) -> Self {
        Self::Arg(name.into(), Cmp::Gt, value.into())
    }
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
#[derive(Clone, Debug, Data)]
pub struct State {
    pub name: String,
    pub play: Play,
    pub transitions: Vec<(String, Condition)>,
    /// Seconds to fade into this state.
    pub fade: f32,
    pub looping: bool,
    pub speed: f32,
    pub paused: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            name: String::new(),
            play: Play::default(),
            transitions: vec![],
            fade: 0.,
            looping: true,
            speed: 1.,
            paused: false,
        }
    }
}
impl State {
    pub fn clip(name: impl Into<String>, clip: impl Into<String>) -> Self {
        Self::new(name, Play::Clip(clip.into()))
    }
    pub fn blend(name: impl Into<String>, blend: Blend) -> Self {
        Self::new(name, Play::Blend(blend))
    }
    pub fn once(mut self) -> Self {
        self.looping = false;
        self
    }
    pub fn speed(mut self, speed: f32) -> Self {
        assert!(speed.is_finite());
        self.speed = speed;
        self
    }
    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

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
#[derive(Default, Clone, Debug, Data)]
pub struct Animator {
    pub playback: Playback,
    from_motion: Vec3,
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
    pub fn state_named(&self, name: &str) -> Option<&State> {
        self.states.iter().find(|s| s.name == name)
    }
    pub fn state_mut(&mut self, name: &str) -> Option<&mut State> {
        self.states.iter_mut().find(|s| s.name == name)
    }
    pub fn blend_mut(&mut self, name: &str) -> Option<&mut Blend> {
        match &mut self.state_mut(name)?.play {
            Play::Blend(b) => Some(b),
            _ => None,
        }
    }
    fn advance(
        &mut self,
        pose: &mut Pose,
        model: &Model,
        dt: Option<f32>,
        scratch: &mut Vec<f32>,
        rest: &[f32],
        ik: Option<&Ik>,
    ) -> Result<(), String> {
        let resample = dt.is_none();
        let dt = dt.unwrap_or(0.);
        let state = self
            .states
            .get(self.current as usize)
            .ok_or("animator current state out of range")?;
        if !resample && self.since == 0. && !state.looping && state.speed < 0. {
            pose.phase = 1.;
        }
        let finished = if state.speed < 0. {
            pose.phase <= 0.
        } else {
            pose.phase >= 1.
        };
        let edge = (!resample
            && !state.paused
            && (state.looping || finished)
            && self.fade_time >= self.fade_duration)
            .then(|| {
                state
                    .transitions
                    .iter()
                    .find(|(to, c)| to != &state.name && c.matches(&self.params))
            })
            .flatten();
        let next = edge
            .map(|(to, _)| {
                self.states
                    .iter()
                    .position(|s| &s.name == to)
                    .ok_or_else(|| format!("unknown state `{to}`"))
            })
            .transpose()?;
        let was_looping = state.looping;
        let state = &self.states[next.unwrap_or(self.current as usize)];
        if !state.speed.is_finite() || !state.fade.is_finite() || state.fade < 0. {
            return Err("invalid state speed/fade".into());
        }
        let pair = match &state.play {
            Play::Clip(n) => {
                let c = clip(model, n)?;
                (c, c, 0.)
            }
            Play::Blend(b) => b.pair(model, &self.params)?,
        };
        let root = self.playback.root(model)?;
        // Stage the transition and fade; IK must succeed before the machine commits.
        let from = next.map(|_| pose.local.clone());
        let from_motion = if next.is_some() {
            self.playback.root_motion()
        } else {
            self.from_motion
        };
        let mut since = if next.is_some() { 0. } else { self.since };
        let mut fade_time = if next.is_some() { 0. } else { self.fade_time };
        let fade_duration = if next.is_some() {
            state.fade
        } else {
            self.fade_duration
        };
        if next.is_some() {
            // Looping locomotion retains phase; entering/leaving a one-shot starts afresh.
            if !state.looping || !was_looping {
                pose.phase = if state.speed < 0. { 1. } else { 0. };
            }
        }
        if !state.paused {
            since += dt;
        }
        advance_pair(
            pair,
            pose,
            if state.paused { 0. } else { dt * state.speed },
            scratch,
            rest,
            model,
            root,
            state.looping,
        );
        if !resample
            && !state.paused
            && !state.looping
            && (state.speed == 0. || math::lerp(pair.0.duration(), pair.1.duration(), pair.2) == 0.)
        {
            pose.phase = if state.speed < 0. { 0. } else { 1. };
        }
        let outgoing = from.as_ref().unwrap_or(&self.from);
        if fade_time < fade_duration {
            if !state.paused {
                fade_time = (fade_time + dt).min(fade_duration);
            }
            if outgoing.len() == pose.local.len() {
                let weight = fade_time / fade_duration;
                mix_pose(outgoing, &mut pose.local, weight);
                pose.root_motion = if state.paused {
                    Vec3::ZERO
                } else {
                    from_motion.lerp(pose.root_motion, weight)
                };
                // Frozen outgoing pose emits no markers. Incoming events become audible above half weight.
                if weight <= 0.5 {
                    pose.crossed.clear();
                }
            }
        }
        if let Some(ik) = ik {
            solve_ik(model, &mut pose.local, ik)?;
        }
        if resample {
            return Ok(());
        }
        if let Some(next) = next {
            self.current = next as u32;
            self.from = from.unwrap();
        }
        self.from_motion = from_motion;
        self.since = since;
        self.fade_time = fade_time;
        self.fade_duration = fade_duration;
        Ok(())
    }
}
#[derive(Default, Clone, Debug, Data)]
pub struct Ik {
    pub chain: [String; 3],
    pub target: Vec3,
    pub pole: Vec3,
    pub weight: f32,
}
#[derive(Default)]
struct Runtime {
    entities: Vec<Entity>,
    stamp: Option<[u64; 6]>,
    output: Motion,
    sockets: SocketCache,
    rigs: BTreeMap<String, Rig>,
    scratch: Vec<f32>,
    pending: Pose,
    errors: BTreeMap<Entity, String>,
}
fn runtime(w: &World) -> std::cell::RefMut<'_, Runtime> {
    std::cell::RefMut::map(w.animation_runtime.borrow_mut(), |slot| {
        slot.get_or_insert_with(|| Box::new(Runtime::default()))
            .downcast_mut::<Runtime>()
            .expect("animation runtime type")
    })
}
struct Rig {
    model: std::sync::Weak<Model>,
    rest: Vec<f32>,
    bounds: [f32; 6],
}
// Only games that register animation data link its generated Pose schema.
macro_rules! controller {
    ($($ty:ty),+) => { $(impl Component for $ty {
        const NAME: &'static str = stringify!($ty);
        fn register(w: &mut World) {
            w.register::<Pose>();
        }
        fn accepts(w: &World, e: Entity) -> bool {
            if conflicts::<Self>(w, e) {
                w.log(format_args!("animation #{}: Animation, Blend and Animator are alternatives", e.index()));
                false
            } else { true }
        }
    })+ };
}
controller!(Animation, Blend, Animator, Ik);
impl Rig {
    fn new(asset: &crate::asset::ModelAsset) -> Self {
        Self {
            model: std::sync::Arc::downgrade(&asset.model),
            rest: bind_pose(&asset.model),
            bounds: asset.bounds,
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
fn wrap(time: f32, duration: f32) -> f32 {
    if duration > 0. {
        time - math::floor(time / duration) * duration
    } else {
        0.
    }
}
// glTF names need not be unique. Resolve the first match in parent-first order.
fn named_node(model: &Model, name: &str) -> Option<u32> {
    let mut matches = model
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.name == name);
    let first = matches.next()?.0 as u32;
    if matches.next().is_none() {
        return Some(first);
    }
    node_order(model)
        .into_iter()
        .find(|&i| model.nodes[i as usize].name == name)
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
        if new > old {
            math::floor((old - mark) / duration) != math::floor((new - mark) / duration)
        } else {
            math::floor((mark - old) / duration) != math::floor((mark - new) / duration)
        }
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
#[allow(clippy::too_many_arguments)]
fn advance_pair(
    (a, b, weight): (&Clip, &Clip, f32),
    p: &mut Pose,
    dt: f32,
    scratch: &mut Vec<f32>,
    rest: &[f32],
    model: &Model,
    root: Option<u32>,
    looping: bool,
) {
    let duration = math::lerp(a.duration(), b.duration(), weight);
    let old = p.phase;
    let next = old + if duration > 0. { dt / duration } else { 0. };
    p.phase = if looping {
        wrap(next, 1.)
    } else {
        next.clamp(0., 1.)
    };
    let next = if looping { next } else { p.phase };
    sample(a, p.phase * a.duration(), rest, &mut p.local);
    p.root_motion = extract_motion(
        a,
        old * a.duration(),
        next * a.duration(),
        looping,
        model,
        root,
        rest,
        &mut p.local,
    ) * (1. - weight);
    if weight > 0. {
        sample(b, p.phase * b.duration(), rest, scratch);
        p.root_motion += extract_motion(
            b,
            old * b.duration(),
            next * b.duration(),
            looping,
            model,
            root,
            rest,
            scratch,
        ) * weight;
        mix_pose(&p.local, scratch, weight);
        p.local.copy_from_slice(scratch);
    }
    markers(
        a,
        &[],
        old * a.duration(),
        next * a.duration(),
        looping,
        &mut p.crossed,
    );
    if weight > 0. {
        markers(
            b,
            &[],
            old * b.duration(),
            next * b.duration(),
            looping,
            &mut p.crossed,
        );
    }
}
#[allow(clippy::too_many_arguments)]
fn extract_motion(
    c: &Clip,
    old: f32,
    next: f32,
    looping: bool,
    model: &Model,
    root: Option<u32>,
    rest: &[f32],
    local: &mut [f32],
) -> Vec3 {
    let Some(root) = root else { return Vec3::ZERO };
    let start = root as usize * 10;
    local[start..start + 3].copy_from_slice(&rest[start..start + 3]);
    let Some(track) = c
        .tracks
        .iter()
        .find(|t| t.node == root && matches!(t.path, TrackPath::Translation))
    else {
        return Vec3::ZERO;
    };
    let duration = c.duration();
    let position = |time| {
        let t = if looping { wrap(time, duration) } else { time };
        let mut v = Vec3::from_slice(&value(track, t));
        if looping && duration > 0. {
            v += (Vec3::from_slice(&value(track, duration)) - Vec3::from_slice(&value(track, 0.)))
                * math::floor(time / duration);
        }
        v
    };
    let delta = position(next) - position(old);
    model.nodes[root as usize].parent.map_or(delta, |parent| {
        joint_matrix(model, local, parent).transform_vector3(delta)
    })
}
/// Deterministic model-local joint matrix, composing only the requested ancestor chain.
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
        ids[i] = named_node(model, name).ok_or_else(|| format!("unknown IK joint `{name}`"))?;
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
/// Evaluate clips explicitly after game-authored parameters, at most once per fixed tick.
pub fn step(w: &mut World) -> Motion {
    let stamp = |w: &World| {
        [
            w.tick(),
            w.membership::<Animation>(),
            w.membership::<Blend>(),
            w.membership::<Animator>(),
            w.revision::<Ik>(),
            w.revision::<Mesh>(),
        ]
    };
    let mut cached = runtime(w);
    // Check before queries, sorting or sampling. Redelivery still resamples at the saved clock.
    if cached.stamp == Some(stamp(w))
        && cached.errors.is_empty()
        && cached.rigs.iter().all(|(name, rig)| {
            w.assets.models.get(name).is_some_and(|model| {
                rig.model
                    .upgrade()
                    .is_some_and(|old| std::sync::Arc::ptr_eq(&old, &model.model))
            })
        })
    {
        return cached.output.clone();
    }
    if w.storage::<Animation>().is_none_or(|s| s.is_empty())
        && w.storage::<Blend>().is_none_or(|s| s.is_empty())
        && w.storage::<Animator>().is_none_or(|s| s.is_empty())
        && w.storage::<Ik>().is_none_or(|s| s.is_empty())
    {
        return Motion::default();
    }
    let mut runtime = std::mem::take(&mut *cached);
    drop(cached);
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
        .extend(w.query::<&Ik>().iter().map(|(e, _)| e));
    runtime
        .errors
        .retain(|e, _| w.contains(*e) && runtime.entities.contains(e));
    runtime.entities.sort_unstable();
    runtime.entities.dedup();
    let mut redelivered_models = std::collections::BTreeSet::new();
    for &e in &runtime.entities {
        let result = (|| {
            let mesh = w.get::<Mesh>(e).ok_or("animation needs a mesh")?;
            let Mesh::Asset(name) = &*mesh else {
                return Err("animation needs Mesh::asset".into());
            };
            if !w.assets.declared.contains(name) {
                return Err(format!("animation model `{name}` must be in Game::ASSETS"));
            }
            let asset = w
                .assets
                .models
                .get(name)
                .ok_or("animation model not loaded")?;
            let model = asset.model.clone();
            if model.nodes.len() > 256 {
                return Err("animation supports at most 256 imported nodes".into());
            }
            let rig = match runtime.rigs.get_mut(name) {
                Some(rig) => rig,
                None => runtime
                    .rigs
                    .entry(name.clone())
                    .or_insert_with(|| Rig::new(asset)),
            };
            if !rig
                .model
                .upgrade()
                .is_some_and(|old| std::sync::Arc::ptr_eq(&old, &model))
            {
                *rig = Rig::new(asset);
                redelivered_models.insert(name.clone());
            }
            let redelivered = redelivered_models.contains(name);
            let stepped = w
                .get::<Pose>(e)
                .is_some_and(|p| p.stepped == Some(w.tick()));
            let dt = if stepped { 0. } else { w.dt() };
            if !redelivered && stepped {
                return Ok(());
            }
            if !redelivered
                && w.get::<Pose>(e).is_some_and(|p| {
                    p.local.len() != rig.rest.len() || p.previous.len() != rig.rest.len()
                })
            {
                return Err(format!("saved pose does not match model `{name}`"));
            }
            drop(mesh);
            if !w.has::<Pose>(e) || redelivered {
                w.insert(
                    e,
                    Pose {
                        previous: rig.rest.clone(),
                        local: rig.rest.clone(),
                        bounds: rig.bounds,
                        ..w.get::<Pose>(e).map(|p| p.clone()).unwrap_or_default()
                    },
                );
            }
            let mut pose = w.get_mut::<Pose>(e).unwrap();
            let p = &mut runtime.pending;
            p.local.clone_from(&pose.local);
            p.phase = pose.phase;
            p.stepped = pose.stepped;
            p.crossed.clear();
            p.root_motion = Vec3::ZERO;
            if let Some(mut a) = w.get_mut::<Animation>(e) {
                if !a.time.is_finite() || !a.speed.is_finite() {
                    return Err("non-finite animation clock".into());
                }
                let c = clip(&model, &a.clip)?;
                let root = a.playback.root(&model)?;
                let duration = c.duration();
                let old = if !a.looping && a.speed < 0. && a.time == 0. && !a.sampled {
                    duration
                } else {
                    a.time
                };
                let next = old + dt * a.speed;
                let time = if a.looping {
                    wrap(next, duration)
                } else {
                    next.clamp(0., duration)
                };
                let next = if a.looping { next } else { time };
                sample(c, time, &rig.rest, &mut p.local);
                markers(c, &a.markers, old, next, a.looping, &mut p.crossed);
                p.root_motion = extract_motion(
                    c,
                    old,
                    next,
                    a.looping,
                    &model,
                    root,
                    &rig.rest,
                    &mut p.local,
                );
                if let Some(ik) = w.get::<Ik>(e) {
                    solve_ik(&model, &mut p.local, &ik)?;
                }
                if !stepped {
                    a.time = time;
                    a.sampled = true;
                    a.playback.record(p);
                }
            } else if let Some(mut b) = w.get_mut::<Blend>(e) {
                let root = b.playback.root(&model)?;
                advance_pair(
                    b.pair(&model, &[])?,
                    p,
                    dt,
                    &mut runtime.scratch,
                    &rig.rest,
                    &model,
                    root,
                    true,
                );
                if let Some(ik) = w.get::<Ik>(e) {
                    solve_ik(&model, &mut p.local, &ik)?;
                }
                if !stepped {
                    b.playback.record(p);
                }
            } else if let Some(mut a) = w.get_mut::<Animator>(e) {
                a.advance(
                    p,
                    &model,
                    (!stepped).then_some(dt),
                    &mut runtime.scratch,
                    &rig.rest,
                    w.get::<Ik>(e).as_deref(),
                )?;
                if !stepped {
                    a.playback.record(p);
                }
            } else {
                p.local.copy_from_slice(&rig.rest);
                if let Some(ik) = w.get::<Ik>(e) {
                    solve_ik(&model, &mut p.local, &ik)?;
                }
            }
            // Commit history only after successful sampling/IK. Birth has no bind predecessor.
            if pose.stepped.is_none() || redelivered {
                pose.previous.clone_from(&p.local);
            } else {
                let pose = &mut *pose;
                pose.previous.clone_from(&pose.local);
            }
            std::mem::swap(&mut pose.local, &mut p.local);
            pose.bounds = rig.bounds;
            pose.phase = p.phase;
            if !stepped {
                pose.root_motion = p.root_motion;
                pose.crossed.clone_from(&p.crossed);
            }
            pose.stepped = Some(w.tick());
            let p = &*pose;
            for marker in p.crossed.iter().filter(|_| !stepped) {
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
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            if let Some(mut p) = w.get_mut::<Pose>(e) {
                p.crossed.clear();
                p.root_motion = Vec3::ZERO;
            }
            if let Some(mut a) = w.get_mut::<Animation>(e) {
                a.playback.clear();
            }
            if let Some(mut b) = w.get_mut::<Blend>(e) {
                b.playback.clear();
            }
            if let Some(mut a) = w.get_mut::<Animator>(e) {
                a.playback.clear();
            }
            if runtime.errors.get(&e) != Some(&error) {
                w.log(format_args!("animation #{}: {error}", e.index()));
                runtime.errors.insert(e, error);
            }
        } else {
            runtime.errors.remove(&e);
        }
    }
    // Reuse unshared output; retained results keep their immutable tick snapshot.
    if runtime.output.0.as_mut().and_then(Arc::get_mut).is_none() {
        runtime.output.0 = Some(Arc::new(Vec::with_capacity(runtime.entities.len())));
    }
    let output = Arc::get_mut(runtime.output.0.as_mut().unwrap()).unwrap();
    let mut len = 0;
    for &e in &runtime.entities {
        if let Some(p) = w.get::<Pose>(e) {
            if let Some((entity, name, playback)) = output.get_mut(len) {
                *entity = e;
                if let Some(current) = w.name(e) {
                    current.clone_into(name.get_or_insert_with(String::new));
                } else {
                    *name = None;
                }
                playback.record(&p);
            } else {
                output.push((
                    e,
                    w.name(e).map(String::from),
                    Playback {
                        crossed: p.crossed.clone(),
                        root_motion: p.root_motion,
                        motion_root: None,
                    },
                ));
            }
            len += 1;
        }
    }
    output.truncate(len);
    runtime.stamp = Some(stamp(w));
    let output = runtime.output.clone();
    *self::runtime(w) = runtime;
    output
}
/// Agent inspection only: all unique skin joints in imported-node order (models cap nodes at 256).
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
    if local.len() != model.nodes.len() * 10
        || pose
            .as_ref()
            .is_some_and(|p| p.previous.len() != local.len())
    {
        return Err(format!("saved pose does not match model `{name}`"));
    }
    let global = Mat4::from(w.current_global(e).unwrap_or(crate::Affine3A::IDENTITY));
    let mut rows = Vec::new();
    let joints: std::collections::BTreeSet<_> = model
        .skins
        .iter()
        .flat_map(|s| &s.joints)
        .copied()
        .collect();
    for i in joints {
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
    pub fn apply(self, w: &mut World) {
        for (name, a, b, c) in self.0 {
            let Some(e) = w.named(&name) else { continue };
            // Membership is authored too: the saved world may predate this controller.
            // Keep dynamic playback only when the controller kind still agrees.
            if let Some(fresh) = &a {
                w.remove_component::<Blend>(e);
                w.remove_component::<Animator>(e);
                if !w.has::<Animation>(e) && !w.insert(e, fresh.clone()) {
                    w.log(format_args!(
                        "animation carry `{name}`: controller insert refused"
                    ));
                }
            } else if let Some(fresh) = &b {
                w.remove_component::<Animation>(e);
                w.remove_component::<Animator>(e);
                if !w.has::<Blend>(e) && !w.insert(e, fresh.clone()) {
                    w.log(format_args!(
                        "animation carry `{name}`: controller insert refused"
                    ));
                }
            } else if let Some(fresh) = &c {
                w.remove_component::<Animation>(e);
                w.remove_component::<Blend>(e);
                if !w.has::<Animator>(e) && !w.insert(e, fresh.clone()) {
                    w.log(format_args!(
                        "animation carry `{name}`: controller insert refused"
                    ));
                }
            }
            if let (Some(fresh), Some(mut old)) = (a, w.get_mut::<Animation>(e)) {
                old.clip = fresh.clip;
                old.speed = fresh.speed;
                old.looping = fresh.looping;
                old.markers = fresh.markers;
                old.playback.motion_root = fresh.playback.motion_root;
            }
            if let (Some(fresh), Some(mut old)) = (b, w.get_mut::<Blend>(e)) {
                old.clips = fresh.clips;
                old.parameter = fresh.parameter;
                old.playback.motion_root = fresh.playback.motion_root;
            }
            if let (Some(fresh), Some(mut old)) = (c, w.get_mut::<Animator>(e)) {
                old.playback.motion_root = fresh.playback.motion_root;
                let name = old.state();
                if let Some(current) = fresh.states.iter().position(|s| s.name == name) {
                    if crate::hash::of(&old.states) != crate::hash::of(&fresh.states) {
                        // An edited blend starts from the carried pose, never from bind.
                        if let Some(pose) = w.get::<Pose>(e) {
                            old.from.clone_from(&pose.local);
                            old.from_motion = pose.root_motion;
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

/// The model executor's specialized entity inspection; never called by the core dispatcher.
pub fn inspect(w: &World, e: Entity, pose: bool) -> Result<String, String> {
    if pose {
        pose_json(w, e)
    } else {
        status_json(w, e)
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
mod tests;
