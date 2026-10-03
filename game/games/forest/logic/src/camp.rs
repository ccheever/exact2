//! The day/night cycle, the sky and the campfire whose light is the safe zone.
use exact_game::*;

/// Seconds of daylight, including dusk.
pub const DAY: f32 = 80.0;
/// Seconds of night.
pub const NIGHT: f32 = 50.0;
pub const PERIOD: f32 = DAY + NIGHT;
const DUSK: f32 = 10.0;
const DAWN: f32 = 8.0;
pub const MAX_FUEL: f32 = 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct Cycle {
    /// The current day, from 1.
    pub day: u32,
    /// Seconds into the current day/night period.
    pub t: f32,
    /// Nights survived.
    pub survived: u32,
}

impl Cycle {
    pub fn night(&self) -> bool {
        self.t >= DAY
    }
    /// 0 in full daylight, 1 in full night.
    pub fn darkness(&self) -> f32 {
        if self.t < DAWN {
            1.0 - math::smoothstep(0.0, DAWN, self.t)
        } else if self.t < DAY - DUSK {
            0.0
        } else if self.t < DAY {
            math::smoothstep(DAY - DUSK, DAY, self.t)
        } else {
            1.0
        }
    }
    pub fn phase(&self) -> &'static str {
        if self.night() {
            "Night"
        } else if self.t >= DAY - DUSK {
            "Dusk"
        } else if self.t < DAWN && self.day > 1 {
            "Dawn"
        } else {
            "Day"
        }
    }
    /// Seconds until the next day/night change.
    pub fn left(&self) -> f32 {
        if self.night() {
            PERIOD - self.t
        } else {
            DAY - self.t
        }
    }
}

/// Fuel in the fire, 0..MAX_FUEL. The safe radius grows with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct Fire {
    pub fuel: f32,
    pub fed: u32,
}

impl Fire {
    pub fn radius(&self) -> f32 {
        if self.fuel <= 0.0 {
            0.0
        } else {
            4.0 + self.fuel * 0.2
        }
    }
}

const DAY_SKY: ([f32; 3], [f32; 3], [f32; 3]) =
    ([0.22, 0.42, 0.78], [0.58, 0.68, 0.72], [0.05, 0.06, 0.03]);
const NIGHT_SKY: ([f32; 3], [f32; 3], [f32; 3]) = (
    [0.004, 0.006, 0.018],
    [0.01, 0.014, 0.03],
    [0.002, 0.002, 0.003],
);

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        math::lerp(a[0], b[0], t),
        math::lerp(a[1], b[1], t),
        math::lerp(a[2], b[2], t),
    ]
}

pub fn environment(dark: f32) -> Environment {
    let fog = mix3([0.5, 0.6, 0.62], [0.004, 0.006, 0.012], dark);
    Environment {
        zenith: mix3(DAY_SKY.0, NIGHT_SKY.0, dark),
        horizon: mix3(DAY_SKY.1, NIGHT_SKY.1, dark),
        ground: mix3(DAY_SKY.2, NIGHT_SKY.2, dark),
        ambient: math::lerp(0.6, 0.35, dark),
        fog: Some(Fog {
            color: Some(fog),
            ..Fog::new(math::lerp(0.007, 0.03, dark), 0.04)
        }),
        bloom: Some(Bloom {
            threshold: 1.0,
            intensity: math::lerp(0.12, 0.3, dark),
            radius: 1.5,
        }),
        ..Environment::default()
    }
}

pub fn build(w: &mut World) {
    w.insert_resource(Cycle {
        day: 1,
        t: DAWN,
        survived: 0,
    });
    w.insert_resource(Fire { fuel: 60.0, fed: 0 });
    w.insert_resource(environment(0.0));
    w.spawn_named(
        "sun",
        (
            Transform::at(30.0, 60.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ),
    );
    // The second directional light is an unshadowed fill: the moon.
    w.spawn_named(
        "moon",
        (
            Transform::at(-21.0, 51.0, -24.0).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                color: [0.55, 0.65, 1.0],
                illuminance: 0.0,
                shadows: false,
            },
        ),
    );
    w.spawn_named(
        "fire",
        (
            // High enough that the ground at the edge of the light is not grazed,
            // and above the flame so the trees, not the logs, throw the shadows.
            Transform::at(0.0, 3.0, 0.0),
            PointLight {
                color: [1.0, 0.55, 0.2],
                intensity: 0.0,
                range: 1.0,
            },
            LightShadows,
        ),
    );
    w.spawn_named(
        "flame",
        (
            Transform::at(0.0, 0.45, 0.0),
            Mesh::sphere(0.45),
            Material {
                color: [1.0, 0.5, 0.1, 1.0],
                ..Material::glow([6.0, 2.2, 0.4])
            },
        ),
    );
    w.spawn_named(
        "embers",
        (
            Transform::at(0.0, 0.6, 0.0),
            Emitter {
                gravity: Vec3::new(0.0, 1.5, 0.0),
                speed: 1.2,
                spread: 0.35,
                size: [0.09, 0.0],
                bound: [-3.0, -1.0, -3.0, 3.0, 6.0, 3.0],
                ..Emitter::sparks().rate(40.0).lifetime(1.6).seed(9)
            },
        ),
    );
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        let (s, c) = math::sin_cos(a);
        w.spawn((
            Transform {
                position: Vec3::new(c * 0.55, 0.15, s * 0.55),
                rotation: Quat::from_rotation_y(-a) * Quat::from_rotation_x(1.3),
                scale: Vec3::ONE,
            },
            Mesh::cylinder(0.12, 1.1),
            Material::rgb(0.2, 0.12, 0.06),
        ));
    }
    for k in 0..10 {
        let a = k as f32 / 10.0 * std::f32::consts::TAU;
        let (s, c) = math::sin_cos(a);
        w.spawn((
            Transform::at(c * 1.25, 0.1, s * 1.25),
            Mesh::sphere(0.22),
            Material::rgb(0.3, 0.3, 0.32),
        ));
    }
}

/// Torches on poles along a spiral out from the fire, each a point light.
pub fn torches(w: &mut World, count: u32) {
    for k in 0..count {
        let a = k as f32 * 2.4;
        let r = 7.0 + k as f32 * 0.9;
        let (s, c) = math::sin_cos(a);
        let (x, z) = w
            .resource::<crate::forest::Grove>()
            .resolve(c * r, s * r, 0.6);
        let y = crate::forest::height(x, z);
        w.spawn((
            Transform::at(x, y + 1.0, z),
            Mesh::cylinder(0.08, 2.0),
            Material::rgb(0.2, 0.13, 0.07),
        ));
        w.spawn((
            Transform::at(x, y + 2.1, z),
            Mesh::sphere(0.14),
            Material {
                color: [1.0, 0.6, 0.2, 1.0],
                ..Material::glow([5.0, 2.0, 0.4])
            },
            PointLight {
                color: [1.0, 0.6, 0.25],
                intensity: 50_000.0,
                range: 8.0,
            },
        ));
    }
}

/// Advance the clock, burn fuel and present the sky and the fire.
/// Returns true on the tick a night is survived.
pub fn step(w: &mut World, player: Vec3) -> bool {
    let dt = w.dt();
    let (dawned, dark, night) = {
        let mut c = w.resource_mut::<Cycle>();
        c.t += dt;
        let mut dawned = false;
        if c.t >= PERIOD {
            c.t -= PERIOD;
            c.day += 1;
            c.survived += 1;
            dawned = true;
        }
        (dawned, c.darkness(), c.night())
    };
    let radius = {
        let mut f = w.resource_mut::<Fire>();
        f.fuel = (f.fuel - dt * if night { 0.6 } else { 0.4 }).max(0.0);
        f.radius()
    };
    // The sky only changes at dusk and dawn; equal writes would still re-light it.
    let env = environment(dark);
    if *w.resource::<Environment>() != env {
        *w.resource_mut::<Environment>() = env;
    }
    let t = w.resource::<Cycle>().t;
    let sun_angle = (t / DAY).clamp(0.0, 1.0) * std::f32::consts::PI;
    let (s, c) = math::sin_cos(sun_angle);
    let sun = Vec3::new(c * 0.8, s.max(0.08) * 1.2, 0.35).normalize();
    *w.require_mut::<Transform>("sun") = Transform::at(
        player.x + sun.x * 60.0,
        sun.y * 60.0,
        player.z + sun.z * 60.0,
    )
    .looking_at(Vec3::new(player.x, 0.0, player.z), Vec3::Y);
    {
        // The sun fades out at dusk; its shadows stop once it no longer lights anything.
        let mut light = w.require_mut::<DirectionalLight>("sun");
        let next = 9000.0 * (0.35 + 0.65 * s.max(0.0)) * (1.0 - dark);
        let shadows = dark < 0.95;
        if light.illuminance != next || light.shadows != shadows {
            light.illuminance = next;
            light.shadows = shadows;
        }
    }
    {
        let mut moon = w.require_mut::<DirectionalLight>("moon");
        let next = 900.0 * dark;
        if moon.illuminance != next {
            moon.illuminance = next;
        }
    }
    // The fire: light radius follows fuel; intensity keeps the edge equally lit.
    {
        let mut light = w.require_mut::<PointLight>("fire");
        light.range = radius * 1.7 + 0.01;
        light.intensity = if radius > 0.0 {
            3000.0 * radius * radius
        } else {
            0.0
        };
    }
    let flame = 0.25 + radius / 24.0 * 0.9;
    w.require_mut::<Transform>("flame").scale = Vec3::splat(flame);
    w.require_mut::<Emitter>("embers").rate = radius * 3.0;
    dawned
}
