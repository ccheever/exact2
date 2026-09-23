//! Saved emission intent; particles are derived into the renderer's own scratch.
use crate::{math, Component, Data, Vec3, World};

/// Hard world-wide live-particle budget, admitted in entity order each tick.
pub const PARTICLE_BUDGET: u32 = 65_536;
/// Local emission volume, oriented along positive Y.
#[derive(Clone, Copy, Debug, Default, Data)]
pub enum Shape {
    /// A single point.
    #[default]
    Point,
    /// Uniform volume inside a sphere of this radius.
    Sphere(f32),
    /// Cone with base radius and height, pointing along Y.
    Cone(f32, f32),
}
/// Interpolation of the two authored lifetime keys.
#[derive(Clone, Copy, Debug, Default, Data)]
pub enum Ease {
    /// Constant rate of change.
    #[default]
    Linear,
    /// Cubic smoothstep.
    Smooth,
}
impl Ease {
    fn at(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::Smooth => t * t * (3. - 2. * t),
        }
    }
}
/// One tick's admitted births, never individual particle positions or velocities.
#[derive(Clone, Debug, Default, Data)]
pub struct Birth {
    /// Emitter age in ticks at birth.
    pub tick: u64,
    /// Number admitted on that tick.
    pub count: u32,
    /// Counter-based random stream key for these births.
    pub key: u64,
    /// Lifetime at admission; changing the authoring value affects new births.
    pub lifetime: f32,
}
/// Saved, hashed emitter state. The renderer never mutates it.
#[derive(Debug, Default, Data)]
pub struct EmitterState {
    /// Completed emission ticks.
    pub age: u64,
    /// Next random stream counter, including refused births.
    pub stream: u64,
    /// Fractional rate accumulator.
    pub fraction: f64,
    /// Queued one-shot births.
    pub burst: u32,
    /// Number of burst requests consumed.
    pub bursts: u64,
    /// Currently alive particles, also the count derived at alpha=1.
    pub alive: u32,
    /// Cumulative refused births because the world budget was full.
    pub dropped: u64,
    /// Compact live birth batches, including one tick of interpolation history.
    pub births: Vec<Birth>,
}
impl Clone for EmitterState {
    fn clone(&self) -> Self {
        Self {
            births: self.births.clone(),
            ..*self
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.age = source.age;
        self.stream = source.stream;
        self.fraction = source.fraction;
        self.burst = source.burst;
        self.bursts = source.bursts;
        self.alive = source.alive;
        self.dropped = source.dropped;
        self.births.clone_from(&source.births);
    }
}

/// A texture-free particle emitter. Add Ambient to exclude it from clock settling.
#[derive(Debug, Component)]
pub struct Emitter {
    /// Local spawn volume.
    pub shape: Shape,
    /// Births per second.
    pub rate: f32,
    /// Seconds alive.
    pub lifetime: f32,
    /// Initial velocity magnitude.
    pub speed: f32,
    /// Cone half-angle in radians around positive Y.
    pub spread: f32,
    /// Local acceleration.
    pub gravity: Vec3,
    /// Linear velocity drag per second.
    pub drag: f32,
    /// Diameter at birth and death, in world units.
    pub size: [f32; 2],
    /// Linear RGBA at birth and death.
    pub color: [[f32; 4]; 2],
    /// Lifetime key interpolation.
    pub ease: Ease,
    /// Initial deterministic random key.
    pub seed: u64,
    /// Authored local AABB, used by layout without deriving particles.
    pub bound: [f32; 6],
    /// Equal-depth translucent ordering; smaller layers draw first.
    pub layer: i32,
    /// Additive blending instead of straight alpha.
    pub additive: bool,
    /// Emit new particles; existing particles continue until death.
    pub running: bool,
    /// Saved emission history, independent of renderer residency.
    pub state: EmitterState,
}
impl Clone for Emitter {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            ..*self
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.shape = source.shape;
        self.rate = source.rate;
        self.lifetime = source.lifetime;
        self.speed = source.speed;
        self.spread = source.spread;
        self.gravity = source.gravity;
        self.drag = source.drag;
        self.size = source.size;
        self.color = source.color;
        self.ease = source.ease;
        self.seed = source.seed;
        self.bound = source.bound;
        self.layer = source.layer;
        self.additive = source.additive;
        self.running = source.running;
        self.state.clone_from(&source.state);
    }
}

impl Default for Emitter {
    fn default() -> Self {
        Self {
            shape: Shape::Point,
            rate: 30.,
            lifetime: 1.,
            speed: 3.,
            spread: 0.5,
            gravity: Vec3::new(0., -9.81, 0.),
            drag: 0.,
            size: [0.08, 0.],
            color: [[1., 0.5, 0.05, 1.], [1., 0.1, 0., 0.]],
            ease: Ease::Linear,
            seed: 0,
            bound: [-4., -8., -4., 4., 4., 4.],
            layer: 0,
            additive: true,
            running: true,
            state: EmitterState::default(),
        }
    }
}
impl Emitter {
    /// A small upward orange spark fountain.
    pub fn sparks() -> Self {
        Self::default()
    }
    /// Set births per second.
    pub fn rate(mut self, rate: f32) -> Self {
        self.rate = rate;
        self
    }
    /// Set lifetime in seconds.
    pub fn lifetime(mut self, seconds: f32) -> Self {
        self.lifetime = seconds;
        self
    }
    /// Set the deterministic seed.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
    /// Queue a one-shot burst for the next emission tick, in addition to the rate.
    pub fn burst(mut self, count: u32) -> Self {
        self.state.burst = self.state.burst.saturating_add(count);
        self
    }
    /// Validate authored parameters before any derivation or tick mutation.
    pub fn validate(&self) -> Result<(), String> {
        let valid = self.rate.is_finite()
            && self.rate >= 0.
            && self.rate <= 1_000_000.
            && self.lifetime.is_finite()
            && self.lifetime > 0.
            && self.lifetime <= 60.
            && self.speed.is_finite()
            && self.spread.is_finite()
            && (0. ..=std::f32::consts::PI).contains(&self.spread)
            && self.gravity.is_finite()
            && self.drag.is_finite()
            && self.drag >= 0.
            && self.size.iter().all(|v| v.is_finite() && *v >= 0.)
            && self.color.iter().flatten().all(|v| v.is_finite())
            && self.bound.iter().all(|v| v.is_finite())
            && (0..3).all(|i| self.bound[i] <= self.bound[i + 3])
            && match self.shape {
                Shape::Point => true,
                Shape::Sphere(r) => r.is_finite() && r >= 0.,
                Shape::Cone(r, h) => r.is_finite() && h.is_finite() && r >= 0. && h >= 0.,
            };
        if valid {
            Ok(())
        } else {
            Err("Emitter: invalid shape, rate, lifetime, motion, keys or bounds".into())
        }
    }
    fn displayed_age(&self, alpha: f32) -> f64 {
        self.state.age.saturating_sub(1) as f64
            + if self.state.age == 0 {
                0.
            } else {
                alpha.clamp(0., 1.) as f64
            }
    }
    /// How many particles [`Self::particles`] visits at this alpha, without deriving
    /// them: presentation that skips an emitter still charges its share of a budget.
    pub fn live(&self, hz: u32, alpha: f32) -> u32 {
        let age = self.displayed_age(alpha);
        let mut remaining = PARTICLE_BUDGET;
        for birth in &self.state.births {
            let seconds = (age - birth.tick as f64) / hz as f64;
            if seconds >= 0. && seconds < birth.lifetime as f64 {
                remaining -= birth.count.min(remaining);
            }
        }
        PARTICLE_BUDGET - remaining
    }
    /// Visit derived live particles at the displayed age; no allocation or mutation.
    /// Alpha interpolates the previous/current completed ticks, as entity poses do.
    pub fn particles(&self, hz: u32, alpha: f32, mut visit: impl FnMut(Particle)) {
        let age = self.displayed_age(alpha);
        let spread_cos = math::cos(self.spread);
        let mut remaining = PARTICLE_BUDGET;
        for birth in &self.state.births {
            let seconds = (age - birth.tick as f64) / hz as f64;
            if seconds < 0. || seconds >= birth.lifetime as f64 {
                continue;
            }
            let t = seconds as f32;
            let (travel, gravity) = if self.drag > 0.0001 {
                let travel = (1. - math::exp(-self.drag * t)) / self.drag;
                (travel, (t - travel) / self.drag)
            } else {
                (t, 0.5 * t * t)
            };
            let u = self.ease.at((t / birth.lifetime).clamp(0., 1.));
            let size = math::lerp(self.size[0], self.size[1], u);
            let color = std::array::from_fn(|i| math::lerp(self.color[0][i], self.color[1][i], u));
            for i in 0..birth.count.min(remaining) {
                let mut key = birth.key.wrapping_add(u64::from(i) * 8);
                let mut random = || {
                    key = key.wrapping_add(1);
                    (mix(key) >> 40) as f32 / 16777216.
                };
                let z = 2. * random() - 1.;
                let angle = random() * std::f32::consts::TAU;

                let origin = match self.shape {
                    Shape::Point => Vec3::ZERO,
                    Shape::Sphere(r) => {
                        let radial = math::sqrt((1. - z * z).max(0.));
                        let direction =
                            Vec3::new(radial * math::cos(angle), z, radial * math::sin(angle));
                        direction * (r * math::powf(random(), 1. / 3.))
                    }
                    Shape::Cone(r, h) => {
                        let y = random();
                        let r = r * y * math::sqrt(random());
                        Vec3::new(r * math::cos(angle), h * y, r * math::sin(angle))
                    }
                };
                let y = 1. - random() * (1. - spread_cos);
                let r = math::sqrt((1. - y * y).max(0.));
                let v = Vec3::new(r * math::cos(angle), y, r * math::sin(angle)) * self.speed;
                visit(Particle {
                    position: origin + v * travel + self.gravity * gravity,
                    size,
                    color,
                });
            }
            remaining -= birth.count.min(remaining);
        }
    }
}
fn mix(mut n: u64) -> u64 {
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    n ^ (n >> 31)
}
/// Unsaved output of deterministic derivation, consumed directly by the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Local position.
    pub position: Vec3,
    /// Quad diameter.
    pub size: f32,
    /// Linear straight RGBA.
    pub color: [f32; 4],
}
/// Advance saved emitter state once from Game::tick. Existing particles reserve
/// capacity first; births share remaining capacity in entity order. Invalid emitters
/// report a named journal line and do not emit. No renderer or wall clock is involved.
pub fn step(w: &World) {
    let hz = w.hz() as f64;
    let mut alive = 0u32;
    for (_, e) in w.query::<&mut Emitter>().iter() {
        if e.validate().is_err() {
            continue;
        }
        if !(e.running && e.rate > 0.)
            && e.state.alive == 0
            && e.state.burst == 0
            && e.state.births.is_empty()
        {
            continue;
        }
        e.state.age += 1;
        let age = e.state.age;
        e.state.births.retain(|b| {
            (age.saturating_sub(1) - b.tick.min(age.saturating_sub(1))) as f64 / hz
                < b.lifetime as f64
        });
        e.state.alive = e
            .state
            .births
            .iter()
            .filter(|b| (age - b.tick) as f64 / hz < b.lifetime as f64)
            .map(|b| b.count)
            .sum();
        alive = alive.saturating_add(e.state.alive);
    }
    let mut left = PARTICLE_BUDGET.saturating_sub(alive);
    for (entity, e) in w.query::<&mut Emitter>().iter() {
        if let Err(error) = e.validate() {
            w.log(format!("{} #{}", error, entity.index()));
            continue;
        }
        let state = &mut e.state;
        if e.running {
            state.fraction += e.rate as f64 / hz;
        }
        let rate = state.fraction as u32;
        state.fraction -= rate as f64;
        let wanted = rate.saturating_add(state.burst);
        state.bursts += u64::from(state.burst > 0);
        state.burst = 0;
        let count = wanted.min(left);
        if count > 0 {
            state.births.push(Birth {
                tick: state.age,
                count,
                key: mix(e.seed).wrapping_add(state.stream),
                lifetime: e.lifetime,
            });
        }
        state.stream = state.stream.wrapping_add(u64::from(wanted) * 8);
        state.alive += count;
        state.dropped = state.dropped.saturating_add(u64::from(wanted - count));
        left -= count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn particle_invariant_hoisting_preserves_pre_r6_float_bits() {
        // Receipts from e0ab1904's evaluator, including the random draws discarded by Point.
        let pins: std::collections::BTreeMap<String, String> =
            crate::json::from_str(include_str!("../tests/pins.json")).unwrap();
        for (shape, key) in [
            (Shape::Point, "emitter-point"),
            (Shape::Sphere(2.), "emitter-sphere"),
            (Shape::Cone(2., 3.), "emitter-cone"),
        ] {
            let e = Emitter {
                shape,
                drag: 0.4,
                ease: Ease::Smooth,
                state: EmitterState {
                    age: 4,
                    births: vec![
                        Birth {
                            tick: 0,
                            count: 8,
                            key: 99,
                            lifetime: 0.1,
                        },
                        Birth {
                            tick: 2,
                            count: 7,
                            key: 123,
                            lifetime: 0.2,
                        },
                    ],
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut values = Vec::new();
            for alpha in [0., 0.5, 1.] {
                e.particles(60, alpha, |p| {
                    values.extend(p.position.to_array());
                    values.push(p.size);
                    values.extend(p.color);
                });
            }
            assert_eq!(format!("0x{:016x}", crate::hash::of(&values)), pins[key]);
        }
    }
    #[test]
    fn saved_batches_derive_identical_shapes_drag_and_lifetime_edges() {
        for shape in [Shape::Point, Shape::Sphere(2.), Shape::Cone(2., 3.)] {
            let mut w = World::new(60, 7);
            let id = w.spawn(Emitter {
                shape,
                drag: 0.4,
                ease: Ease::Smooth,
                ..Emitter::sparks().rate(0.).lifetime(0.1).seed(99).burst(8)
            });
            for _ in 0..7 {
                step(&w);
            }
            let e = w.get::<Emitter>(id).unwrap();
            let restored: Emitter = crate::bin::from_slice(&crate::bin::to_vec(&*e)).unwrap();
            for alpha in [0., 0.5, 1.] {
                let mut a = Vec::new();
                let mut b = Vec::new();
                e.particles(60, alpha, |p| a.push(p));
                restored.particles(60, alpha, |p| b.push(p));
                assert_eq!(a, b);
                assert_eq!(e.live(60, alpha) as usize, a.len());
                assert!(a.iter().all(|p| p.position.is_finite()));
                if alpha == 1. {
                    assert_eq!(a.len(), e.state.alive as usize);
                }
            }
        }
    }
}
