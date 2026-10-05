//! Presentation playback. `ShownClips` holds clips at times `Game::present` derives from
//! simulation state; renderers and inspection sample it from whichever model
//! has arrived — declared, streamed or loaded on sight — so it follows arrival
//! while nothing a tick reads ever depends on load order.
use super::{clip, mix_pose, named_node, sample, wrap};
use crate::{asset::Model, Entity, Mesh, World};

/// The model drawn for `name`: any that has arrived, declared, streamed or
/// loaded on sight, where `World::model` answers only declarations.
/// Presentation only (renderers, sockets drawn on a `ShownClips` rig): a tick that
/// calls it panics, because load order must never reach the simulation.
pub fn drawn_model<'w>(w: &'w World, name: &str) -> Option<&'w Model> {
    assert!(
        !w.in_tick,
        "animation::drawn_model in a tick: simulation reads declared models (World::model)"
    );
    w.assets.models.get(name).map(|asset| asset.model.as_ref())
}

/// Drawn-only playback for a model entity, written by `Game::present`: one or
/// more clips at the clip times present derives from saved causes (ticks
/// since a step began, distance walked, the last shot's tick). Any arrived
/// model plays it, `Game::STREAMED` ones and models loaded on sight included,
/// because nothing simulated reads it: no clock is saved, no marker crosses,
/// no root motion moves the entity, and `animation::socket` keeps the
/// simulated pose. Until the model arrives the entity draws nothing (as any
/// streamed model); the frame it lands, it draws at present's clip time. It
/// draws in place of a simulated `Pose` on the same entity. Animation that
/// gameplay reads (root motion, markers, sockets a tick queries) stays a
/// simulated `Animation`, `Blend` or `Animator` on a `Game::ASSETS` model.
///
/// ```
/// # use exact_game::{animation::ShownClips, Entity, Present};
/// fn present(p: &mut Present<'_>, soldier: Entity, walked: f32, since_shot: f32) {
///     // Walk by distance (a 1.6 m stride), with the recoil clip mixed over it.
///     let shown = ShownClips::clip("walk", walked / 1.6)
///         .and("recoil", since_shot, 1. - (since_shot / 0.3).min(1.))
///         .once();
///     p.insert(soldier, shown);
/// }
/// ```
#[derive(Clone, Debug, Default, PartialEq, crate::Presentation)]
pub struct ShownClips {
    /// The first clip at full weight, then each later one mixed over the
    /// result by its weight.
    pub clips: Vec<ShownClip>,
    /// A node whose translation stays at its bind pose, as a simulated
    /// `motion_root`'s does: the clip walks in place while the simulation
    /// moves the entity.
    pub in_place: Option<String>,
}
/// One clip of a [`ShownClips`] pose.
#[derive(Clone, Debug, Default, PartialEq, crate::Data)]
pub struct ShownClip {
    pub clip: String,
    /// Clip seconds at this boundary.
    pub time: f32,
    /// Clip seconds per world second: the previous tick, which the renderer
    /// interpolates from, samples at `time - speed * dt`.
    pub speed: f32,
    /// Mixed over the clips before it; the first clip's is ignored.
    pub weight: f32,
    /// Wrap past the clip's end (else hold its ends).
    pub looping: bool,
}
impl ShownClips {
    /// One looping clip at `seconds`, advancing at one clip second per second.
    pub fn clip(name: impl Into<String>, seconds: f32) -> Self {
        Self::default().and(name, seconds, 1.)
    }
    /// Mix another looping clip over the pose so far by `weight` in [0, 1].
    pub fn and(mut self, name: impl Into<String>, seconds: f32, weight: f32) -> Self {
        self.clips.push(ShownClip {
            clip: name.into(),
            time: seconds,
            speed: 1.,
            weight,
            looping: true,
        });
        self
    }
    /// The last clip's rate in clip seconds per world second, for the
    /// previous tick's sample.
    pub fn speed(mut self, speed: f32) -> Self {
        if let Some(last) = self.clips.last_mut() {
            last.speed = speed;
        }
        self
    }
    /// Draw `node`'s translation at its bind pose, the clip walking in place.
    pub fn in_place(mut self, node: impl Into<String>) -> Self {
        self.in_place = Some(node.into());
        self
    }
    /// Hold the last clip at its ends instead of wrapping.
    pub fn once(mut self) -> Self {
        if let Some(last) = self.clips.last_mut() {
            last.looping = false;
        }
        self
    }
}

/// Reusable buffers for sampling [`ShownClips`] poses: ten floats per imported
/// node, as `Pose`.
#[derive(Default)]
pub struct ShownPose {
    /// The pose one tick before this boundary.
    pub previous: Vec<f32>,
    /// The pose at this boundary.
    pub local: Vec<f32>,
    scratch: Vec<f32>,
}
impl ShownPose {
    /// Sample `e`'s `ShownClips` over `rest` (its model's bind pose). Ok(false)
    /// when there is nothing to show: no `ShownClips`, or its model has not
    /// arrived. A named error for a missing clip, a non-model mesh or a
    /// non-finite time; the caller draws the simulated pose instead.
    /// Presentation only: a tick that calls it panics.
    pub fn sample(&mut self, w: &World, e: Entity, rest: &[f32]) -> Result<bool, String> {
        let Some(shown) = w.get::<ShownClips>(e) else {
            return Ok(false);
        };
        let mesh = w.get::<Mesh>(e).ok_or("ShownClips needs a model Mesh")?;
        let Mesh::Asset(name) = &*mesh else {
            return Err("ShownClips needs Mesh::asset".into());
        };
        let Some(model) = drawn_model(w, name) else {
            return Ok(false);
        };
        if rest.len() != model.nodes.len() * 10 {
            return Err(format!("bind pose does not match model `{name}`"));
        }
        let pinned = match &shown.in_place {
            Some(node) => Some(
                named_node(model, node)
                    .ok_or_else(|| format!("ShownClips: unknown node `{node}`"))?
                    as usize
                    * 10,
            ),
            None => None,
        };
        let dt = w.dt();
        for (back, previous) in [(0., false), (dt, true)] {
            let out = if previous {
                &mut self.previous
            } else {
                &mut self.local
            };
            out.clear();
            out.extend_from_slice(rest);
            for (i, c) in shown.clips.iter().enumerate() {
                let clip = clip(model, &c.clip)?;
                let duration = clip.duration();
                let time = c.time - back * c.speed;
                if !time.is_finite() {
                    return Err(format!("ShownClips clip `{}`: non-finite time", c.clip));
                }
                let time = if c.looping {
                    wrap(time, duration)
                } else {
                    time.clamp(0., duration)
                };
                if i == 0 {
                    sample(clip, time, rest, out);
                } else {
                    sample(clip, time, rest, &mut self.scratch);
                    let weight = if c.weight.is_finite() {
                        c.weight.clamp(0., 1.)
                    } else {
                        0.
                    };
                    mix_pose(out, &mut self.scratch, weight);
                    std::mem::swap(out, &mut self.scratch);
                }
            }
            if let Some(at) = pinned {
                out[at..at + 3].copy_from_slice(&rest[at..at + 3]);
            }
        }
        Ok(true)
    }
}

/// Entity inspection: the clips and whether they draw, wait for their
/// model or are refused (and why).
pub(super) fn status_json(w: &World, e: Entity) -> Result<String, String> {
    let Some(shown) = w.get::<ShownClips>(e) else {
        return Ok(String::new());
    };
    let clips = crate::json::to_string(&*shown).map_err(|e| e.to_string())?;
    let quote = crate::values::quote;
    let state = match w.get::<Mesh>(e).as_deref() {
        Some(Mesh::Asset(name)) => match drawn_model(w, name) {
            None => quote("waiting for its model"),
            Some(model) => match ShownPose::default().sample(w, e, &super::bind_pose(model)) {
                Ok(_) => quote("drawn"),
                Err(error) => quote(&error),
            },
        },
        _ => quote("ShownClips needs Mesh::asset"),
    };
    Ok(format!(
        ",\"shown\":{{\"clips\":{clips},\"state\":{state}}}"
    ))
}
