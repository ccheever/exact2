//! Skinned characters from code: a bone hierarchy with lengths and radii becomes a
//! smooth-weighted capsule mesh, its skeleton, and procedural clips (a gait cycle,
//! idle breathing, a flinch). The result is an ordinary [`Model`] for
//! [`World::generated_model`], so `Animator`, `Blend`, `Layers`, sockets, saves and
//! hashes treat it like a baked one. All math is the engine's portable `libm`, so a
//! rig and its clips are bit-identical on every host.
//!
//! Assumptions: the rest pose faces +Z with +Y up; limbs are chains of bones from
//! root to tip whose rest pose hangs down in the Y–Z plane, and gait clips swing
//! them about each joint's X axis; the first spine bone carries the legs and is
//! lowered onto the planted feet. Bones rest unrotated, so clip rotations are
//! relative to the bind pose (which also makes them usable as additive deltas).
//!
//! ```
//! use exact_game::{rig::{self, Gait, Rig}, *};
//! let rig = Rig::humanoid(1.8);
//! let model = rig.model([
//!     rig.idle("idle"),
//!     rig.walk("walk", Gait::walk(1.4)),
//!     rig.walk("run", Gait::run(4.0)),
//!     rig.flinch("flinch"),
//! ]);
//! let mut w = World::new(60, 0);
//! let mesh = w.generated_model("hero.model", model).unwrap();
//! w.spawn_named("hero", (Transform::default(), mesh, Animator::new([
//!     rig::locomotion("move", "idle", [(1.4, "walk"), (4.0, "run")]),
//! ])));
//! rig::drive(&w, "hero", "move", 2.0);
//! ```
use crate::animation::{Blend, State};
use crate::asset::{
    Clip, Interpolation, MaterialData, MeshData, Model, Node, Skin, Track, TrackPath,
};
use crate::{math, Animator, Mat4, Quat, Target, Vec3, World};
use std::f32::consts::{PI, TAU};

/// One bone in the rest pose, in model space (metres, +Y up, facing +Z).
#[derive(Clone, Debug)]
pub struct Bone {
    /// Unique node name; sockets and layer masks use it.
    pub name: String,
    /// Index of the parent bone, added earlier.
    pub parent: Option<usize>,
    /// Joint position; the bone rotates about it.
    pub head: Vec3,
    /// End of the bone's skin; `head` for a joint without geometry.
    pub tail: Vec3,
    /// Skin radius at head and tail; zero draws nothing for this bone.
    pub radius: [f32; 2],
    /// Linear RGBA vertex colour of this bone's skin.
    pub color: [f32; 4],
}

/// Which way a limb swings in a gait cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limb {
    /// Upper, lower and foot: swings, lifts in swing, keeps the foot level.
    Leg,
    /// Upper and lower (and hand): furthest forward at its phase, elbow bent.
    Arm,
}

#[derive(Clone, Debug)]
struct Chain {
    limb: Limb,
    bones: Vec<usize>,
    /// Fraction of the cycle at which this limb is furthest forward.
    phase: f32,
}

/// A skeleton under construction. Bones are added parents first.
#[derive(Clone, Debug)]
pub struct Rig {
    bones: Vec<Bone>,
    chains: Vec<Chain>,
    spine: Vec<usize>,
    sides: u32,
}

/// Gait parameters. A clip authored at `speed` covers `stride` metres per cycle
/// (two steps for a biped), so playback at `speed` keeps planted feet still.
#[derive(Clone, Copy, Debug)]
pub struct Gait {
    /// Ground speed the clip is authored for, m/s.
    pub speed: f32,
    /// Distance covered per cycle, metres.
    pub stride: f32,
    /// Fraction of a cycle each foot is planted: above one half both feet share
    /// the ground (a walk), below it the body flies between steps (a run).
    pub duty: f32,
    /// Peak knee bend in the swing, radians.
    pub lift: f32,
    /// Arm swing as a fraction of the leg swing.
    pub arms: f32,
    /// Forward lean of the spine, radians.
    pub lean: f32,
}
impl Gait {
    /// A walk at `speed` m/s with a stride that grows with it.
    pub fn walk(speed: f32) -> Self {
        Self {
            speed,
            stride: (0.9 + 0.35 * speed).min(2.2),
            duty: 0.6,
            lift: 0.55,
            arms: 0.6,
            lean: 0.04,
        }
    }
    /// A run: longer strides, high knees, a forward lean.
    pub fn run(speed: f32) -> Self {
        Self {
            speed,
            stride: (1.2 + 0.4 * speed).min(3.6),
            duty: 0.35,
            lift: 1.25,
            arms: 1.0,
            lean: 0.18,
        }
    }
    /// Seconds per cycle.
    pub fn period(&self) -> f32 {
        self.stride / self.speed
    }
}

impl Default for Rig {
    fn default() -> Self {
        Self::new()
    }
}

impl Rig {
    /// An empty skeleton; add a root bone first.
    pub fn new() -> Self {
        Self {
            bones: Vec::new(),
            chains: Vec::new(),
            spine: Vec::new(),
            sides: 10,
        }
    }
    /// The bones in order, parents first; each is checked when added.
    pub fn bones(&self) -> &[Bone] {
        &self.bones
    }
    /// Facets around each capsule, clamped to 6..=64 (default 10).
    pub fn sides(&mut self, sides: u32) -> &mut Self {
        self.sides = sides.clamp(6, 64);
        self
    }
    fn index(&self, name: &str) -> usize {
        self.bones
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| panic!("rig: no bone `{name}`"))
    }
    /// Add a bone from `head` to `tail` under `parent` (added earlier), its skin
    /// tapering from `radius[0]` to `radius[1]`. A rig holds at most 255 bones.
    pub fn bone(
        &mut self,
        name: &str,
        parent: Option<&str>,
        head: Vec3,
        tail: Vec3,
        radius: [f32; 2],
        color: [f32; 4],
    ) -> &mut Self {
        assert!(
            self.bones.iter().all(|b| b.name != name),
            "rig: bone `{name}` twice"
        );
        assert!(self.bones.len() < 255, "rig: at most 255 bones");
        assert!(
            head.is_finite()
                && tail.is_finite()
                && radius.iter().all(|r| r.is_finite() && *r >= 0.),
            "rig: bone `{name}` needs finite geometry"
        );
        let parent = parent.map(|p| self.index(p));
        self.bones.push(Bone {
            name: name.into(),
            parent,
            head,
            tail,
            radius,
            color,
        });
        self
    }
    /// Mark a limb chain (root to tip) for gait clips. A leg touches down at `phase`
    /// (a fraction of the cycle); an arm is furthest forward at `phase`.
    pub fn limb(&mut self, limb: Limb, bones: &[&str], phase: f32) -> &mut Self {
        assert!(bones.len() >= 2, "rig: a limb needs at least two bones");
        assert!(phase.is_finite(), "rig: limb phase must be finite");
        let bones = bones.iter().map(|b| self.index(b)).collect();
        self.chains.push(Chain { limb, bones, phase });
        self
    }
    /// Mark the spine (hips to head) that breathes, leans and flinches.
    pub fn spine(&mut self, bones: &[&str]) -> &mut Self {
        self.spine = bones.iter().map(|b| self.index(b)).collect();
        self
    }
    /// A biped `height` metres tall, facing +Z, with hands named `hand_l`/`hand_r`
    /// for sockets. Bones: root, hips, spine, chest, neck, head, and per side
    /// thigh/shin/foot and upper_arm/forearm/hand.
    pub fn humanoid(height: f32) -> Self {
        let h = height / 1.8;
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * h;
        let (skin, cloth, dark) = (
            [0.85, 0.62, 0.48, 1.],
            [0.2, 0.36, 0.7, 1.],
            [0.16, 0.14, 0.18, 1.],
        );
        let mut r = Self::new();
        r.bone("root", None, Vec3::ZERO, Vec3::ZERO, [0., 0.], cloth)
            .bone(
                "hips",
                Some("root"),
                v(0., 0.95, 0.),
                v(0., 1.1, 0.),
                [0.15 * h, 0.15 * h],
                dark,
            )
            .bone(
                "spine",
                Some("hips"),
                v(0., 1.1, 0.),
                v(0., 1.3, 0.),
                [0.15 * h, 0.16 * h],
                cloth,
            )
            .bone(
                "chest",
                Some("spine"),
                v(0., 1.3, 0.),
                v(0., 1.47, 0.),
                [0.17 * h, 0.15 * h],
                cloth,
            )
            .bone(
                "neck",
                Some("chest"),
                v(0., 1.47, 0.),
                v(0., 1.56, 0.),
                [0.06 * h, 0.06 * h],
                skin,
            )
            .bone(
                "head",
                Some("neck"),
                v(0., 1.6, 0.),
                v(0., 1.74, 0.),
                [0.11 * h, 0.1 * h],
                skin,
            );
        for (side, x) in [("l", 1.), ("r", -1.)] {
            let n = |b: &str| format!("{b}_{side}");
            r.bone(
                &n("thigh"),
                Some("hips"),
                v(0.1 * x, 0.95, 0.),
                v(0.1 * x, 0.52, 0.),
                [0.08 * h, 0.065 * h],
                dark,
            )
            .bone(
                &n("shin"),
                Some(&n("thigh")),
                v(0.1 * x, 0.52, 0.),
                v(0.1 * x, 0.09, 0.),
                [0.06 * h, 0.045 * h],
                dark,
            )
            .bone(
                &n("foot"),
                Some(&n("shin")),
                v(0.1 * x, 0.06, -0.03),
                v(0.1 * x, 0.04, 0.16),
                [0.05 * h, 0.04 * h],
                [0.1, 0.08, 0.06, 1.],
            )
            .bone(
                &n("upper_arm"),
                Some("chest"),
                v(0.21 * x, 1.44, 0.),
                v(0.23 * x, 1.17, 0.),
                [0.055 * h, 0.048 * h],
                cloth,
            )
            .bone(
                &n("forearm"),
                Some(&n("upper_arm")),
                v(0.23 * x, 1.17, 0.),
                v(0.24 * x, 0.92, 0.02),
                [0.045 * h, 0.04 * h],
                skin,
            )
            .bone(
                &n("hand"),
                Some(&n("forearm")),
                v(0.24 * x, 0.92, 0.02),
                v(0.24 * x, 0.83, 0.03),
                [0.045 * h, 0.035 * h],
                skin,
            );
            let phase = if x > 0. { 0. } else { 0.5 };
            r.limb(Limb::Leg, &[&n("thigh"), &n("shin"), &n("foot")], phase)
                .limb(
                    Limb::Arm,
                    &[&n("upper_arm"), &n("forearm"), &n("hand")],
                    phase + 0.5,
                );
        }
        r.spine(&["hips", "spine", "chest", "neck", "head"]);
        r
    }
    /// A four-legged animal `length` metres nose to tail, facing +Z: a body, neck,
    /// head and tail, and front/hind legs `leg_fl`… with upper/lower/paw bones,
    /// in a walk sequence (hind left, fore left, hind right, fore right).
    pub fn quadruped(length: f32) -> Self {
        let s = length / 1.4;
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * s;
        let (fur, belly, dark) = (
            [0.62, 0.38, 0.18, 1.],
            [0.8, 0.66, 0.5, 1.],
            [0.2, 0.13, 0.08, 1.],
        );
        let mut r = Self::new();
        r.bone("root", None, Vec3::ZERO, Vec3::ZERO, [0., 0.], fur)
            .bone(
                "pelvis",
                Some("root"),
                v(0., 0.62, -0.38),
                v(0., 0.64, -0.05),
                [0.16 * s, 0.17 * s],
                fur,
            )
            .bone(
                "body",
                Some("pelvis"),
                v(0., 0.64, -0.05),
                v(0., 0.66, 0.36),
                [0.17 * s, 0.18 * s],
                belly,
            )
            .bone(
                "neck",
                Some("body"),
                v(0., 0.68, 0.36),
                v(0., 0.86, 0.56),
                [0.09 * s, 0.07 * s],
                fur,
            )
            .bone(
                "head",
                Some("neck"),
                v(0., 0.88, 0.58),
                v(0., 0.84, 0.8),
                [0.1 * s, 0.06 * s],
                fur,
            )
            .bone(
                "tail",
                Some("pelvis"),
                v(0., 0.64, -0.4),
                v(0., 0.5, -0.78),
                [0.05 * s, 0.02 * s],
                dark,
            );
        for (name, x, z, parent, phase) in [
            ("leg_hl", 1., -0.33, "pelvis", 0.),
            ("leg_fl", 1., 0.33, "body", 0.25),
            ("leg_hr", -1., -0.33, "pelvis", 0.5),
            ("leg_fr", -1., 0.33, "body", 0.75),
        ] {
            let n = |b: &str| format!("{name}_{b}");
            let x = 0.12 * x;
            r.bone(
                &n("upper"),
                Some(parent),
                v(x, 0.6, z),
                v(x, 0.33, z),
                [0.06 * s, 0.045 * s],
                fur,
            )
            .bone(
                &n("lower"),
                Some(&n("upper")),
                v(x, 0.33, z),
                v(x, 0.07, z),
                [0.04 * s, 0.03 * s],
                fur,
            )
            .bone(
                &n("paw"),
                Some(&n("lower")),
                v(x, 0.05, z),
                v(x, 0.03, z + 0.06),
                [0.04 * s, 0.035 * s],
                dark,
            );
            r.limb(Limb::Leg, &[&n("upper"), &n("lower"), &n("paw")], phase);
        }
        r.spine(&["pelvis", "body", "neck", "head"]);
        r
    }

    /// The skinned model: one node per bone (rest rotation identity, so clips are
    /// rotations about each joint), one skin, and one mesh of capsules whose
    /// vertices near a joint blend smoothly between the bone and its parent.
    pub fn model(&self, clips: impl IntoIterator<Item = Clip>) -> Model {
        assert!(!self.bones.is_empty(), "rig: no bones");
        let mut nodes: Vec<Node> = self
            .bones
            .iter()
            .map(|b| {
                let local = b.head - b.parent.map_or(Vec3::ZERO, |p| self.bones[p].head);
                Node {
                    name: b.name.clone(),
                    parent: b.parent.map(|p| p as u32),
                    transform: Mat4::from_translation(local).to_cols_array(),
                    ..Default::default()
                }
            })
            .collect();
        nodes.push(Node {
            name: "skin".into(),
            mesh: Some(0),
            skin: Some(0),
            ..Default::default()
        });
        let skin = Skin {
            name: "rig".into(),
            joints: (0..self.bones.len() as u32).collect(),
            inverse_binds: self
                .bones
                .iter()
                .flat_map(|b| Mat4::from_translation(-b.head).to_cols_array())
                .collect(),
        };
        let mesh = self.mesh();
        Model {
            bounds: mesh.bounds,
            meshes: vec![mesh],
            materials: vec![MaterialData {
                metallic: 0.,
                roughness: 0.75,
                ..Default::default()
            }],
            nodes,
            skins: vec![skin],
            clips: clips.into_iter().collect(),
            ..Default::default()
        }
    }

    fn mesh(&self) -> MeshData {
        let mut m = MeshData::default();
        let sides = self.sides;
        // Rings: a hemisphere at the head, the tube, a hemisphere at the tail.
        const CAP: u32 = 4;
        const BODY: u32 = 4;
        for (i, b) in self.bones.iter().enumerate() {
            if b.radius[0] <= 0. && b.radius[1] <= 0. {
                continue;
            }
            let axis = b.tail - b.head;
            let length = axis.length();
            let dir = if length > 1e-6 {
                axis / length
            } else {
                Vec3::Y
            };
            let side = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
            let u = dir.cross(side).normalize();
            let w = dir.cross(u);
            let first = (m.positions.len() / 3) as u32;
            let rings = 2 * CAP + BODY + 1;
            for ring in 0..rings {
                // s in [-1, 0] is the head cap, [0, 1] the tube, [1, 2] the tail cap.
                let (t, out, cap) = if ring < CAP {
                    let a = (CAP - ring) as f32 / CAP as f32 * PI * 0.5;
                    (0., math::cos(a), -math::sin(a))
                } else if ring <= CAP + BODY {
                    ((ring - CAP) as f32 / BODY as f32, 1., 0.)
                } else {
                    let a = (ring - CAP - BODY) as f32 / CAP as f32 * PI * 0.5;
                    (1., math::cos(a), math::sin(a))
                };
                let radius = b.radius[0] + (b.radius[1] - b.radius[0]) * t;
                let centre = b.head + axis * t + dir * (cap * radius);
                // A ball (zero-length) bone moves rigidly with itself.
                let (joints, weights) = if length > 1e-6 {
                    self.weights(i, (axis * t + dir * (cap * radius)).dot(dir) / length)
                } else {
                    ([i as u16, 0, 0, 0], [1., 0., 0., 0.])
                };
                for k in 0..sides {
                    let (sn, cs) = math::sin_cos(k as f32 / sides as f32 * TAU);
                    let radial = u * cs + w * sn;
                    let normal = (radial * out + dir * cap).normalize();
                    let p = centre + radial * (radius * out);
                    m.positions.extend(p.to_array());
                    m.normals.extend(normal.to_array());
                    m.uvs
                        .extend([k as f32 / sides as f32, ring as f32 / rings as f32]);
                    m.colors.extend(b.color);
                    m.joints.extend(joints);
                    m.weights.extend(weights);
                }
            }
            for ring in 0..rings - 1 {
                for k in 0..sides {
                    let a = first + ring * sides + k;
                    let b2 = first + ring * sides + (k + 1) % sides;
                    let c = a + sides;
                    let d = b2 + sides;
                    m.indices.extend([a, c, b2, b2, c, d]);
                }
            }
        }
        assert!(!m.indices.is_empty(), "rig: no bone has a radius");
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in m.positions.chunks_exact(3) {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        m.bounds = [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]];
        m
    }

    // Near its head, a vertex shares weight with the parent bone (half at the joint),
    // falling smoothly to the bone alone over the first third of its length.
    fn weights(&self, bone: usize, along: f32) -> ([u16; 4], [f32; 4]) {
        let parent = self.bones[bone]
            .parent
            .filter(|&p| self.bones[p].radius.iter().any(|r| *r > 0.));
        match parent {
            Some(p) => {
                let x = (along / 0.33).clamp(0., 1.);
                let shared = 0.5 * (1. - x * x * (3. - 2. * x));
                ([bone as u16, p as u16, 0, 0], [1. - shared, shared, 0., 0.])
            }
            None => ([bone as u16, 0, 0, 0], [1., 0., 0., 0.]),
        }
    }

    fn sagittal(&self, chain: &Chain) -> Sagittal {
        let v = |b: usize| {
            let d = self.bones[b].tail - self.bones[b].head;
            (d.y, d.z)
        };
        Sagittal {
            upper: v(chain.bones[0]),
            lower: v(chain.bones[1]),
        }
    }
    fn rotation_track(bone: usize, times: &[f32], f: impl Fn(f32) -> Quat) -> Track {
        Track {
            node: bone as u32,
            path: TrackPath::Rotation,
            interpolation: Interpolation::Linear,
            times: times.to_vec(),
            values: times.iter().flat_map(|&t| f(t).to_array()).collect(),
        }
    }
    fn samples(period: f32, count: usize) -> Vec<f32> {
        (0..=count)
            .map(|i| period * i as f32 / count as f32)
            .collect()
    }

    /// A looping gait cycle of `gait.period()` seconds, in place (no root motion),
    /// with `step` markers at each footfall. Each leg touches down at its
    /// [`Rig::limb`] phase, then slides back at a constant rate for `duty` of the
    /// cycle, which keeps it still on the ground while the body moves at
    /// `gait.speed`; it then eases forward with its knee lifted.
    pub fn walk(&self, name: &str, gait: Gait) -> Clip {
        let finite = [
            gait.speed,
            gait.stride,
            gait.duty,
            gait.lift,
            gait.arms,
            gait.lean,
        ];
        assert!(
            finite.iter().all(|v| v.is_finite())
                && gait.speed > 0.
                && gait.stride > 0.
                && gait.duty > 0.
                && gait.duty < 1.,
            "rig: gait `{name}` needs finite values, positive speed and stride, duty in (0, 1)"
        );
        let period = gait.period();
        let times = Self::samples(period, 24);
        let mut tracks = Vec::new();
        let mut markers = Vec::new();
        for chain in &self.chains {
            let cycle = move |t: f32| t / period - chain.phase;
            match chain.limb {
                Limb::Leg => {
                    let leg = self.sagittal(chain);
                    let profile = move |t: f32| leg_profile(gait, wrap(t / period - chain.phase));
                    let pose = move |t: f32| {
                        let (x, knee) = profile(t);
                        (leg.hip_for(x, knee), knee)
                    };
                    tracks.push(Self::rotation_track(chain.bones[0], &times, |t| {
                        Quat::from_rotation_x(pose(t).0)
                    }));
                    tracks.push(Self::rotation_track(chain.bones[1], &times, |t| {
                        Quat::from_rotation_x(pose(t).1)
                    }));
                    if let Some(&foot) = chain.bones.get(2) {
                        // Level: undo the hip and knee rotations above it.
                        tracks.push(Self::rotation_track(foot, &times, |t| {
                            let (hip, knee) = pose(t);
                            Quat::from_rotation_x(-(hip + knee))
                        }));
                    }
                    // A footfall when this foot touches down, at its phase.
                    markers.push((wrap(chain.phase) * period, "step".to_string()));
                }
                Limb::Arm => {
                    let swing = 0.45 * gait.arms;
                    tracks.push(Self::rotation_track(chain.bones[0], &times, |t| {
                        Quat::from_rotation_x(-swing * math::cos(TAU * cycle(t)))
                    }));
                    tracks.push(Self::rotation_track(chain.bones[1], &times, |t| {
                        Quat::from_rotation_x(
                            -0.25 - 0.35 * gait.arms * (0.5 + 0.5 * math::cos(TAU * cycle(t))),
                        )
                    }));
                }
            }
        }
        if let Some(&hips) = self.spine.first() {
            let rest = self.bones[hips].head
                - self.bones[hips]
                    .parent
                    .map_or(Vec3::ZERO, |p| self.bones[p].head);
            // The hips ride on the planted legs: lowered by however far an angled
            // stance leg would lift its foot off the ground, so the lowest planted
            // foot touches it (the trailing one of two rises, a heel-off), highest
            // at mid-stance and lowest with the legs splayed. In a
            // run's flight no leg holds them: they arc linearly from lift-off to
            // the next touchdown.
            let legs: Vec<_> = self
                .chains
                .iter()
                .filter(|c| c.limb == Limb::Leg)
                .map(|c| (self.sagittal(c), c.phase))
                .collect();
            let held = |t: f32| {
                legs.iter()
                    .filter_map(|(leg, phase)| {
                        let c = wrap(t / period - phase);
                        (c < gait.duty).then(|| {
                            let (x, knee) = leg_profile(gait, c);
                            leg.ankle(leg.hip_for(x, knee), knee).0 - leg.ankle(0., 0.).0
                        })
                    })
                    .reduce(f32::min)
            };
            let drops = bridge(
                &times[..times.len() - 1]
                    .iter()
                    .map(|&t| held(t))
                    .collect::<Vec<_>>(),
            );
            tracks.push(Track {
                node: hips as u32,
                path: TrackPath::Translation,
                interpolation: Interpolation::Linear,
                times: times.clone(),
                values: (0..times.len())
                    .flat_map(|i| (rest - Vec3::Y * drops[i % drops.len()]).to_array())
                    .collect(),
            });
            let twist = 0.08 * gait.arms;
            tracks.push(Self::rotation_track(hips, &times, |t| {
                Quat::from_rotation_y(twist * math::cos(TAU * t / period))
            }));
        }
        // The spine bone above the hips leans into the gait and counter-twists.
        if let Some(&upper) = self.spine.get(1) {
            tracks.push(Self::rotation_track(upper, &times, |t| {
                Quat::from_rotation_x(gait.lean)
                    * Quat::from_rotation_y(-0.1 * gait.arms * math::cos(TAU * t / period))
            }));
        }
        markers.sort_by(|a, b| a.0.total_cmp(&b.0));
        Clip {
            name: name.into(),
            tracks,
            markers,
        }
    }
    /// A 3 s breathing loop: the spine and chest rise and settle, the head nods.
    pub fn idle(&self, name: &str) -> Clip {
        let period = 3.;
        let times = Self::samples(period, 24);
        let breath = move |t: f32| math::sin(TAU * t / period);
        let tracks = self
            .spine
            .iter()
            .skip(1)
            .enumerate()
            .map(|(i, &b)| {
                let amount = 0.025 / (i + 1) as f32;
                Self::rotation_track(b, &times, move |t| {
                    Quat::from_rotation_x(-amount * breath(t))
                })
            })
            .collect();
        Clip {
            name: name.into(),
            tracks,
            markers: vec![],
        }
    }
    /// A 0.5 s one-shot recoil for an additive layer: the spine snaps back, then
    /// settles to rest; limbs flare slightly. Rotations are deltas from rest.
    pub fn flinch(&self, name: &str) -> Clip {
        let period = 0.5;
        let times = Self::samples(period, 12);
        let pulse = move |t: f32| {
            let x = t / period;
            math::sin(PI * x.min(1.)) * math::exp(-3. * x)
        };
        let mut tracks: Vec<Track> = self
            .spine
            .iter()
            .skip(1)
            .enumerate()
            .map(|(i, &b)| {
                let amount = 0.35 / (1. + i as f32 * 0.5);
                Self::rotation_track(b, &times, move |t| {
                    Quat::from_rotation_x(-amount * pulse(t))
                })
            })
            .collect();
        for chain in self.chains.iter().filter(|c| c.limb == Limb::Arm) {
            let out = if self.bones[chain.bones[0]].head.x >= 0. {
                1.
            } else {
                -1.
            };
            tracks.push(Self::rotation_track(chain.bones[0], &times, move |t| {
                Quat::from_rotation_z(out * 0.4 * pulse(t))
            }));
        }
        Clip {
            name: name.into(),
            tracks,
            markers: vec![],
        }
    }
}

// Foot placement over one leg's cycle `c` (0 = touchdown): the forward offset of
// the foot from the hip and the knee bend. In stance the foot slides back at a
// constant rate (it is planted while the body moves), in the swing it eases forward.
fn leg_profile(gait: Gait, c: f32) -> (f32, f32) {
    let duty = gait.duty;
    let reach = duty * gait.stride;
    if c < duty {
        let u = c / duty;
        (reach * (0.5 - u), 0.08 * gait.lift * math::sin(PI * u))
    } else {
        let u = (c - duty) / (1. - duty);
        let ease = 0.5 - 0.5 * math::cos(PI * u);
        (
            reach * (ease - 0.5),
            gait.lift * math::powi(math::sin(PI * u), 2),
        )
    }
}

// A cyclic series with gaps, filled by straight lines between the known values on
// either side (wrapping); all-gap series are zero.
fn bridge(values: &[Option<f32>]) -> Vec<f32> {
    let n = values.len() as isize;
    let known: Vec<isize> = (0..n).filter(|&i| values[i as usize].is_some()).collect();
    let (Some(&first), Some(&last)) = (known.first(), known.last()) else {
        return vec![0.; values.len()];
    };
    let at = |k: isize| values[k.rem_euclid(n) as usize].unwrap_or(0.);
    (0..n)
        .map(|i| {
            if let Some(v) = values[i as usize] {
                return v;
            }
            let after = known.iter().copied().find(|&k| k > i).unwrap_or(first + n);
            let before = known
                .iter()
                .copied()
                .rev()
                .find(|&k| k < i)
                .unwrap_or(last - n);
            let u = (i - before) as f32 / (after - before) as f32;
            at(before) + (at(after) - at(before)) * u
        })
        .collect()
}

// A leg seen from the side: the rest upper and lower bone vectors as (y, z).
#[derive(Clone, Copy)]
struct Sagittal {
    upper: (f32, f32),
    lower: (f32, f32),
}
fn rotate((y, z): (f32, f32), a: f32) -> (f32, f32) {
    let (s, c) = math::sin_cos(a);
    (y * c - z * s, y * s + z * c)
}
impl Sagittal {
    // The ankle relative to the hip for a hip and knee rotation about X.
    fn ankle(&self, hip: f32, knee: f32) -> (f32, f32) {
        let low = rotate(self.lower, knee);
        rotate((self.upper.0 + low.0, self.upper.1 + low.1), hip)
    }
    // The hip rotation (nearest zero) that puts the ankle `forward` metres ahead.
    fn hip_for(&self, forward: f32, knee: f32) -> f32 {
        let (y, z) = self.ankle(0., knee);
        let r = math::sqrt(y * y + z * z).max(1e-6);
        let phi = math::atan2(z, y);
        let s = math::asin((forward / r).clamp(-0.95, 0.95));
        let near = |a: f32| {
            let a = wrap((a + PI) / TAU) * TAU - PI;
            (a.abs(), a)
        };
        let (a, b) = (near(s - phi), near(PI - s - phi));
        if a.0 <= b.0 {
            a.1
        } else {
            b.1
        }
    }
}

fn wrap(x: f32) -> f32 {
    x - math::floor(x)
}

/// An Animator state blending `idle` at rest into gait clips keyed by the ground
/// speed (m/s) each was authored at, on the parameter `speed`. Clips share a
/// normalized phase, so feet stay in step across the blend; set it with [`drive`].
pub fn locomotion<const N: usize>(state: &str, idle: &str, gaits: [(f32, &str); N]) -> State {
    let mut gaits = gaits.to_vec();
    gaits.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(
        !gaits.is_empty()
            && gaits.iter().all(|g| g.0.is_finite() && g.0 > 0.)
            && gaits.windows(2).all(|p| p[0].0 < p[1].0),
        "rig: locomotion gaits need distinct finite speeds above zero"
    );
    let mut knots = vec![(0., idle)];
    knots.extend(gaits);
    let blend = Blend {
        axis: 0.,
        clips: knots.iter().map(|(v, n)| (*v, n.to_string())).collect(),
        ..Blend::default()
    }
    .parameter("speed");
    State::blend(state, blend)
}

/// Below this ground speed (m/s) a [`locomotion`] state blends into its idle.
pub const IDLE_BELOW: f32 = 0.05;

/// Play `target`'s [`locomotion`] state `state` at ground speed `speed` (m/s) so
/// planted feet stay planted at every speed: below the slowest gait it plays that
/// gait at `speed / slowest` (blending idle in only under [`IDLE_BELOW`]); between
/// gaits it blends them and corrects the rate for the blended stride; above the
/// fastest it plays faster. Zero and negative speeds play the idle.
/// Reads the clip lengths from the entity's generated or delivered model.
pub fn drive(w: &World, target: impl Target, state: &str, speed: f32) {
    assert!(speed.is_finite(), "rig: invalid speed");
    let label = target.label();
    let e = target
        .entity(w)
        .unwrap_or_else(|| panic!("rig: drive target `{label}` does not exist"));
    let mesh = w.get::<crate::Mesh>(e);
    let model = match mesh.as_deref() {
        Some(crate::Mesh::Asset(name)) => w.model(name),
        _ => None,
    }
    .unwrap_or_else(|| panic!("rig: drive target `{label}` has no loaded model"));
    let mut animator = w.require_mut::<Animator>(e);
    let knots = match animator.state_named(state).map(|s| &s.play) {
        Some(crate::Play::Blend(b)) => b.clips.clone(),
        _ => panic!("rig: `{label}` has no locomotion state `{state}`"),
    };
    let period = |clip: &str| {
        model
            .clips
            .iter()
            .find(|c| c.name == clip)
            .map_or(0., |c| c.duration())
    };
    let (axis, rate) = gait_rate(&knots, &period, speed);
    animator.set("speed", axis);
    let s = animator.state_mut(state).unwrap();
    if s.speed != rate {
        s.speed = rate;
    }
}

// The blend axis and playback rate that make the blended gait cover `speed`.
fn gait_rate(knots: &[(f32, String)], period: &dyn Fn(&str) -> f32, speed: f32) -> (f32, f32) {
    let gaits = &knots[1..];
    let (Some(slowest), Some(fastest)) = (gaits.first(), gaits.last()) else {
        return (0., 1.);
    };
    if speed.is_nan() || speed <= 0. {
        return (0., 1.);
    }
    if speed < slowest.0 {
        if speed < IDLE_BELOW {
            let k = speed / IDLE_BELOW;
            return (slowest.0 * k, 1. + (IDLE_BELOW / slowest.0 - 1.) * k);
        }
        return (slowest.0, speed / slowest.0);
    }
    if speed >= fastest.0 {
        return (fastest.0, speed / fastest.0);
    }
    let hi = gaits.partition_point(|g| g.0 <= speed);
    let (a, b) = (&gaits[hi - 1], &gaits[hi]);
    let k = (speed - a.0) / (b.0 - a.0);
    let (ta, tb) = (period(&a.1), period(&b.1));
    let (sa, sb) = (a.0 * ta, b.0 * tb);
    let duration = ta + (tb - ta) * k;
    let covered = if duration > 0. {
        (sa + (sb - sa) * k) / duration
    } else {
        speed
    };
    (speed, if covered > 0. { speed / covered } else { 1. })
}
