//! The lights a scene draws: each entity's presentation `DrawnLight`, else its
//! simulated light; the sun and fill; and the point and spot lights, selected
//! nearest the camera at each tick end.
use super::{displayed, pose, History, Scene};
use crate::MAX_LIGHTS;
use exact_game::{
    DirectionalLight, DrawnLight, Entity, LightShadows, PointLight, SpotLight, Transform, World,
};
use glam::Vec3;

/// The lights drawn, in entity order: each entity's presentation `DrawnLight`,
/// else its simulated `DirectionalLight`, `PointLight` or `SpotLight`.
fn drawn_lights(w: &World) -> Vec<(Entity, DrawnLight)> {
    let mut out: Vec<(Entity, DrawnLight)> = w
        .query::<&DrawnLight>()
        .iter()
        .map(|(e, l)| (e, *l))
        .collect();
    let swapped = out.len();
    let own = |e: Entity| swapped == 0 || !w.has::<DrawnLight>(e);
    out.extend(
        (w.query::<&DirectionalLight>().iter())
            .filter(|(e, _)| own(*e))
            .map(|(e, l)| (e, DrawnLight::Directional(*l))),
    );
    out.extend(
        (w.query::<&PointLight>().iter())
            .filter(|(e, _)| own(*e))
            .map(|(e, l)| (e, DrawnLight::Point(*l))),
    );
    out.extend(
        (w.query::<&SpotLight>().iter())
            .filter(|(e, _)| own(*e))
            .map(|(e, l)| (e, DrawnLight::Spot(*l))),
    );
    // Stable: an entity carrying two simulated kinds keeps their query order.
    out.sort_by_key(|(e, _)| e.index());
    out
}
// A point light is a spot whose cone is the whole sphere.
#[derive(Clone, Copy)]
pub(super) struct Emitter {
    pub(super) color: [f32; 3],
    pub(super) intensity: f32,
    pub(super) range: f32,
    // Cosines of the inner and outer half-angles; a point light has none.
    pub(super) cone: Option<[f32; 2]>,
    pub(super) shadows: bool,
}
impl Emitter {
    fn point(light: &PointLight, shadows: bool) -> Self {
        Self {
            color: light.color,
            intensity: light.intensity,
            range: light.range,
            cone: None,
            shadows,
        }
    }
    #[allow(clippy::manual_clamp)] // clamp would keep NaN
    fn spot(light: &SpotLight, shadows: bool) -> Self {
        // `max` maps NaN to zero, which `clamp` would keep.
        let outer = light.outer.max(0.).min(std::f32::consts::FRAC_PI_2);
        let outer_cos = exact_game::math::cos(outer);
        // The shader's smoothstep needs a nonempty edge.
        let inner_cos = exact_game::math::cos(light.inner.max(0.).min(outer)).max(outer_cos + 1e-4);
        Self {
            color: light.color,
            intensity: light.intensity,
            range: light.range,
            cone: Some([inner_cos, outer_cos]),
            shadows,
        }
    }
}
pub(super) struct Light {
    pub(super) history: History,
    pub(super) light: Emitter,
    pub(super) lit: Option<exact_game::Lit>,
}
impl Scene {
    /// Retain the drawn lights whose inputs changed, then reselect the drawn
    /// set at the tick end. `versions` are this feed's light revisions.
    pub(super) fn feed_lights(
        &mut self,
        w: &World,
        versions: [u64; 8],
        next_tick: bool,
        moved: bool,
        structure: bool,
        parent_changed: bool,
    ) {
        let old = self.versions;
        let visibility_changed = old.is_none_or(|v| v[6] != versions[6]) || parent_changed;
        let swapped = old.is_none_or(|v| v[7] != versions[7]);
        let lights =
            if old.is_none_or(|v| v[1..] != versions[1..]) || structure || visibility_changed {
                drawn_lights(w)
            } else {
                Vec::new()
            };
        if old.is_none_or(|v| v[1] != versions[1]) || swapped || structure || visibility_changed {
            // The first two posed directional lights: the sun, then a fill.
            let (sun, fill) = (self.sun, self.fill);
            let keep = |e: Entity, t: Transform| {
                [sun, fill]
                    .into_iter()
                    .flatten()
                    .find(|(h, _)| h.entity == e)
                    .map_or(History::new(e, t), |(h, _)| h)
            };
            let mut posed = (lights.iter())
                .filter_map(|&(e, l)| match l {
                    DrawnLight::Directional(s) => Some((e, s)),
                    _ => None,
                })
                .filter(|(e, _)| w.is_visible(*e))
                .filter_map(|(e, s)| pose(w, e).map(|t| (keep(e, t), s)));
            self.sun = posed.next();
            self.fill = posed.next();
        }
        for (history, _) in [&mut self.sun, &mut self.fill].into_iter().flatten() {
            history.update(w, next_tick, parent_changed);
        }
        if old.is_none_or(|v| v[2..] != versions[2..]) || structure || visibility_changed {
            // Whether the entity still casts a point (or spot) light.
            let casts = |e: Entity, spot: bool| {
                let at = lights.partition_point(|(l, _)| l.index() < e.index());
                lights[at..]
                    .iter()
                    .take_while(|(l, _)| l.index() == e.index())
                    .any(|(l, k)| {
                        *l == e
                            && match k {
                                DrawnLight::Spot(_) => spot,
                                DrawnLight::Point(_) => !spot,
                                DrawnLight::Directional(_) => false,
                            }
                    })
            };
            // Compact departures once; keep each kind's histories in entity order
            // and append arrivals. Value-only edits reuse the buffer without sorting.
            let mut kept = 0;
            let mut spots = 0;
            let spot_at = self.first_spot;
            let mut index = 0;
            self.lights.retain(|l| {
                let spot = index >= spot_at;
                index += 1;
                let e = l.history.entity;
                let live = w.is_visible(e) && w.global(e).is_some() && casts(e, spot);
                if live {
                    kept += 1;
                    spots += usize::from(spot);
                }
                live
            });
            let points = kept - spots;
            let mut fresh = Vec::new();
            let mut at = 0;
            let points_of = lights.iter().filter_map(|&(e, l)| match l {
                DrawnLight::Point(p) => Some((e, p)),
                _ => None,
            });
            for (e, light) in points_of {
                let light = &light;
                if !w.is_visible(e) {
                    continue;
                }
                let emitter = Emitter::point(light, w.has::<LightShadows>(e));
                let lit = w.get::<exact_game::Lit>(e).as_deref().cloned();
                if at < points && self.lights[at].history.entity == e {
                    (self.lights[at].light, self.lights[at].lit) = (emitter, lit);
                    at += 1;
                } else if let Some(t) = pose(w, e) {
                    fresh.push((
                        false,
                        Light {
                            history: History::new(e, t),
                            light: emitter,
                            lit,
                        },
                    ));
                }
            }
            let mut at = points;
            let spots_of = lights.iter().filter_map(|&(e, l)| match l {
                DrawnLight::Spot(s) => Some((e, s)),
                _ => None,
            });
            for (e, light) in spots_of {
                let light = &light;
                if !w.is_visible(e) {
                    continue;
                }
                let emitter = Emitter::spot(light, w.has::<LightShadows>(e));
                let lit = w.get::<exact_game::Lit>(e).as_deref().cloned();
                if at < kept && self.lights[at].history.entity == e {
                    (self.lights[at].light, self.lights[at].lit) = (emitter, lit);
                    at += 1;
                } else if let Some(t) = pose(w, e) {
                    fresh.push((
                        true,
                        Light {
                            history: History::new(e, t),
                            light: emitter,
                            lit,
                        },
                    ));
                }
            }
            self.first_spot = points;
            if !fresh.is_empty() {
                let mut all: Vec<_> = self
                    .lights
                    .drain(..)
                    .enumerate()
                    .map(|(i, l)| (i >= points, l))
                    .chain(fresh)
                    .collect();
                all.sort_by_key(|(spot, l)| (*spot, l.history.entity.index()));
                self.first_spot = all.iter().filter(|(spot, _)| !spot).count();
                self.lights.extend(all.into_iter().map(|(_, l)| l));
            }
        }
        if next_tick || moved || structure || visibility_changed || old != Some(versions) {
            self.selected.clear();
            self.attachments.frame(1.);
            let camera = self.camera.map_or(Vec3::ZERO, |(h, _)| {
                displayed(&self.attachments.output, h.entity, h.curr).position
            });
            for (i, light) in self.lights.iter_mut().enumerate() {
                light.history.update(w, next_tick, parent_changed);
                // Eligibility is evaluated at the same tick endpoint as distance.
                let intensity = light.light.intensity
                    * light
                        .lit
                        .as_ref()
                        .map_or(1., |lit| lit.0.value(w.now()).max(0.));
                if !intensity.is_finite()
                    || intensity <= 0.
                    || !light.light.range.is_finite()
                    || light.light.range <= 0.
                {
                    continue;
                }
                let distance = displayed(
                    &self.attachments.output,
                    light.history.entity,
                    light.history.curr,
                )
                .position
                .distance_squared(camera);
                // Selection depends only on current state; list order (points, then
                // spots, each in entity order) breaks ties.
                self.selected.push((distance, i));
            }
            self.selected
                .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            self.dropped = self.selected.len().saturating_sub(MAX_LIGHTS);
            // Frames then fill the output without allocating.
            self.output.reserve(self.selected.len().min(MAX_LIGHTS));
        }
    }
}
