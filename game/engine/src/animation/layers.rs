//! Ordered, masked clip layers over the base controller's pose.
use super::*;

/// One saved overlay. Additive deltas use the imported bind pose as reference.
#[derive(Clone, Debug, Data)]
pub struct Layer {
    pub animation: Animation,
    pub weight: f32,
    pub additive: bool,
    /// Empty means every node; otherwise each named node and its descendants.
    pub mask: Vec<String>,
}
impl Default for Layer {
    fn default() -> Self {
        Self {
            animation: Animation::default(),
            weight: 1.,
            additive: false,
            mask: Vec::new(),
        }
    }
}
impl Layer {
    pub fn new(animation: Animation) -> Self {
        Self {
            animation,
            ..Self::default()
        }
    }
    pub fn additive(mut self) -> Self {
        self.additive = true;
        self
    }
    pub fn weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }
    pub fn mask(mut self, nodes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.mask = nodes.into_iter().map(Into::into).collect();
        self
    }
}
/// Overlays evaluated in order after Animation/Blend/Animator, before IK.
/// May also stand alone over the bind pose. The base owns extracted root motion.
#[derive(Default, Clone, Debug, Data)]
pub struct Layers(pub Vec<Layer>);
impl Component for Layers {
    const NAME: &'static str = "Layers";
    fn register(w: &mut World) {
        w.register::<Pose>();
    }
}
impl Layers {
    pub(super) fn validate(&self, model: &Model) -> Result<(), String> {
        for layer in &self.0 {
            let a = &layer.animation;
            if !layer.weight.is_finite()
                || !(0. ..=1.).contains(&layer.weight)
                || !a.time.is_finite()
                || !a.speed.is_finite()
            {
                return Err("invalid animation layer weight or clock".into());
            }
            clip(model, &a.clip)?;
            if a.motion_root.is_some() {
                return Err(
                    "root motion belongs to the base controller, not an animation layer".into(),
                );
            }
            for name in &layer.mask {
                if named_node(model, name).is_none() {
                    return Err(format!("unknown layer mask node `{name}`"));
                }
            }
        }
        Ok(())
    }
    pub(super) fn apply(
        &mut self,
        model: &Model,
        rest: &[f32],
        pose: &mut Pose,
        dt: f32,
        scratch: &mut Vec<f32>,
        motion_root: Option<u32>,
    ) {
        for layer in &mut self.0 {
            let mask: Vec<_> = layer
                .mask
                .iter()
                .filter_map(|name| named_node(model, name))
                .collect();
            let a = &mut layer.animation;
            let c = clip(model, &a.clip).expect("validated layer");
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
            sample(c, time, rest, scratch);
            a.playback.clear();
            if layer.weight > 0. {
                markers(
                    c,
                    &a.markers,
                    old,
                    if a.looping { next } else { time },
                    a.looping,
                    &mut a.playback.crossed,
                );
                pose.crossed.extend(a.playback.crossed.iter().cloned());
                for track in &c.tracks {
                    if motion_root == Some(track.node)
                        && matches!(track.path, TrackPath::Translation)
                    {
                        continue;
                    }
                    let mut node = Some(track.node);
                    let mut included = layer.mask.is_empty();
                    while let Some(index) = node {
                        let n = &model.nodes[index as usize];
                        included |= mask.contains(&index);
                        node = n.parent;
                    }
                    if !included {
                        continue;
                    }
                    let offset = track.node as usize * 10;
                    let mut base = at(&pose.local[offset..]);
                    let sample = at(&scratch[offset..]);
                    let bind = at(&rest[offset..]);
                    let weight = layer.weight;
                    match track.path {
                        TrackPath::Translation => {
                            base.position = if layer.additive {
                                base.position + (sample.position - bind.position) * weight
                            } else {
                                base.position.lerp(sample.position, weight)
                            }
                        }
                        TrackPath::Rotation => {
                            base.rotation = if layer.additive {
                                (base.rotation
                                    * Quat::IDENTITY
                                        .slerp(bind.rotation.inverse() * sample.rotation, weight))
                                .normalize()
                            } else {
                                base.rotation.slerp(sample.rotation, weight).normalize()
                            }
                        }
                        TrackPath::Scale => {
                            base.scale = if layer.additive {
                                base.scale * Vec3::ONE.lerp(sample.scale / bind.scale, weight)
                            } else {
                                base.scale.lerp(sample.scale, weight)
                            }
                        }
                    }
                    put(&mut pose.local[offset..], base);
                }
            }
            a.time = time;
            a.sampled = true;
        }
    }
}
