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

/// The sky graded through the day: noon, a warm dusk (strongest halfway into
/// the dark) and a blue-black night. Fog hugs the ground and thickens at night.
pub fn environment(dark: f32) -> Environment {
    let glow = 4.0 * dark * (1.0 - dark);
    let toward = |day: [f32; 3], dusk: [f32; 3], night: [f32; 3]| {
        let base = mix3(day, night, dark);
        mix3(base, dusk, glow * 0.55)
    };
    let fog = toward(
        [0.46, 0.55, 0.56],
        [0.36, 0.26, 0.24],
        [0.005, 0.008, 0.016],
    );
    Environment {
        zenith: toward(DAY_SKY.0, [0.16, 0.12, 0.3], NIGHT_SKY.0),
        horizon: toward(DAY_SKY.1, [0.95, 0.42, 0.2], NIGHT_SKY.1),
        ground: toward(DAY_SKY.2, [0.1, 0.05, 0.03], NIGHT_SKY.2),
        ambient: math::lerp(0.55, 0.35, dark),
        exposure: math::lerp(1.0, 1.4, dark),
        fog: Some(Fog {
            color: Some(fog),
            ..Fog::new(math::lerp(0.006, 0.04, dark), 0.12)
        }),
        bloom: Some(Bloom {
            threshold: 1.0,
            intensity: math::lerp(0.14, 0.32, dark),
            radius: 1.6,
        }),
        ..Environment::default()
    }
}

/// The image-based light: the day sky until the sun is mostly gone, then the
/// starry night, at an intensity that steps (each step re-prefilters the map).
fn sky_map(dark: f32) -> EnvironmentMap {
    let night = dark > 0.6;
    let level = if night { dark } else { 1.0 - dark };
    EnvironmentMap {
        texture: if night {
            "sky_night.tex"
        } else {
            "sky_day.tex"
        }
        .into(),
        intensity: math::round(level.max(0.1) * 8.0) / 8.0 * if night { 1.6 } else { 0.9 },
        rgbm: 8.0,
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
    w.insert_resource(sky_map(0.0));
    w.insert_resource(AmbientOcclusion {
        radius: 0.7,
        intensity: 0.9,
    });
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
    // Coals: a low glowing mound the flames rise from.
    w.spawn_named(
        "flame",
        (
            Transform::at(0.0, 0.12, 0.0).with_scale(Vec3::new(1.0, 0.35, 1.0)),
            Mesh::sphere(0.42),
            Material {
                color: [1.0, 0.4, 0.1, 1.0],
                ..Material::glow([5.0, 1.2, 0.15])
            },
        ),
    );
    // Flames in two layers, smoke above them, embers through both.
    let flames = |rate, life, size: [f32; 2], speed, color, seed| Emitter {
        shape: emitter::Shape::Sphere(0.32),
        gravity: Vec3::new(0.0, 2.6, 0.0),
        speed,
        spread: 0.3,
        drag: 0.6,
        size,
        color,
        ease: emitter::Ease::Smooth,
        bound: [-2.0, -0.5, -2.0, 2.0, 4.0, 2.0],
        ..Emitter::sparks().rate(rate).lifetime(life).seed(seed)
    };
    w.spawn_named(
        "flame-core",
        (
            Transform::at(0.0, 0.25, 0.0),
            flames(
                90.0,
                0.45,
                [0.42, 0.08],
                0.9,
                [[1.0, 0.85, 0.45, 1.0], [1.0, 0.35, 0.05, 0.0]],
                11,
            ),
        ),
    );
    w.spawn_named(
        "flame-outer",
        (
            Transform::at(0.0, 0.3, 0.0),
            flames(
                60.0,
                0.75,
                [0.7, 0.15],
                0.7,
                [[1.0, 0.45, 0.08, 0.9], [0.6, 0.08, 0.02, 0.0]],
                12,
            ),
        ),
    );
    w.spawn_named(
        "smoke",
        (
            Transform::at(0.0, 1.1, 0.0),
            Emitter {
                shape: emitter::Shape::Sphere(0.2),
                gravity: Vec3::new(0.25, 0.9, 0.05),
                speed: 0.5,
                spread: 0.4,
                drag: 0.35,
                size: [0.5, 2.6],
                color: [[0.07, 0.065, 0.06, 0.45], [0.05, 0.05, 0.05, 0.0]],
                ease: emitter::Ease::Smooth,
                additive: false,
                bound: [-6.0, -1.0, -6.0, 6.0, 12.0, 6.0],
                ..Emitter::sparks().rate(9.0).lifetime(5.0).seed(13)
            },
        ),
    );
    w.spawn_named(
        "embers",
        (
            Transform::at(0.0, 0.5, 0.0),
            Emitter {
                gravity: Vec3::new(0.0, 1.5, 0.0),
                speed: 1.6,
                spread: 0.45,
                size: [0.06, 0.0],
                color: [[1.0, 0.7, 0.2, 1.0], [1.0, 0.2, 0.0, 0.0]],
                bound: [-3.0, -1.0, -3.0, 3.0, 7.0, 3.0],
                ..Emitter::sparks().rate(30.0).lifetime(2.2).seed(9)
            },
        ),
    );
    // Four charred logs crossed in the coals, a ring of stones, three seat logs.
    for k in 0..4 {
        let a = k as f32 / 4.0 * std::f32::consts::TAU + 0.4;
        w.spawn((
            Transform {
                position: Vec3::new(0.0, 0.2, 0.0),
                rotation: Quat::from_rotation_y(a) * Quat::from_rotation_z(0.35),
                scale: Vec3::splat(0.9),
            },
            Mesh::asset("firelog.model"),
        ));
    }
    for k in 0..11 {
        let a = k as f32 / 11.0 * std::f32::consts::TAU;
        let (s, c) = math::sin_cos(a);
        w.spawn((
            Transform {
                position: Vec3::new(c * 1.2, 0.02, s * 1.2),
                rotation: Quat::from_rotation_y(a * 3.0),
                scale: Vec3::splat(0.38 + 0.06 * math::sin(a * 5.0)),
            },
            Mesh::asset("rock_b.model"),
        ));
    }
    for k in 0..3 {
        let a = k as f32 / 3.0 * std::f32::consts::TAU + 0.9;
        let (s, c) = math::sin_cos(a);
        w.spawn((
            Transform {
                position: Vec3::new(c * 3.4, 0.22, s * 3.4),
                rotation: Quat::from_rotation_y(-a + std::f32::consts::FRAC_PI_2),
                scale: Vec3::new(1.5, 1.3, 1.3),
            },
            Mesh::asset("log.model"),
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
    let map = sky_map(dark);
    if *w.resource::<EnvironmentMap>() != map {
        *w.resource_mut::<EnvironmentMap>() = map;
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
        // Late light warms toward amber as the sun sets.
        let color = mix3(
            [1.0, 0.95, 0.88],
            [1.0, 0.55, 0.3],
            (4.0 * dark * (1.0 - dark)).min(1.0),
        );
        if light.illuminance != next || light.shadows != shadows || light.color != color {
            light.illuminance = next;
            light.shadows = shadows;
            light.color = color;
        }
    }
    {
        let mut moon = w.require_mut::<DirectionalLight>("moon");
        let next = 1800.0 * dark;
        if moon.illuminance != next {
            moon.illuminance = next;
        }
    }
    // The fire: light radius follows fuel; intensity keeps the edge equally lit.
    {
        let mut light = w.require_mut::<PointLight>("fire");
        light.range = radius * 1.7 + 0.01;
        // Flicker: three incommensurate waves, deterministic in world time.
        let now = w.tick_end().seconds() as f32;
        let flicker = 0.86
            + 0.08 * math::sin(now * 11.3)
            + 0.04 * math::sin(now * 23.7 + 1.3)
            + 0.03 * math::sin(now * 4.1 + 0.4);
        light.intensity = if radius > 0.0 {
            3000.0 * radius * radius * flicker
        } else {
            0.0
        };
    }
    let size = (radius / 14.0).clamp(0.15, 1.6);
    let coals = Vec3::new(1.0, 0.35, 1.0) * (0.6 + 0.4 * size);
    if w.require::<Transform>("flame").scale != coals {
        w.require_mut::<Transform>("flame").scale = coals;
    }
    for (name, rate) in [
        ("flame-core", 90.0),
        ("flame-outer", 60.0),
        ("smoke", 9.0),
        ("embers", 30.0),
    ] {
        let next = if radius > 0.0 { rate * size } else { 0.0 };
        if w.require::<Emitter>(name).rate != next {
            w.require_mut::<Emitter>(name).rate = next;
        }
    }
    dawned
}
