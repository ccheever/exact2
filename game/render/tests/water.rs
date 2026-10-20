//! Water (LLP 1139.002): the surface, caustics and shafts, each measured in
//! look frames (LLP 1127.000) of one pool under `Environment::underwater()`,
//! and each beside a fixture that must fail its oracle for the stated reason:
//! a flat quad in place of the surface, a constant in place of the caustics,
//! no shafts. The pool's surface is 6 m up; the camera is 2 m under it (or
//! 1 m over it, above a floor 1 m down); the sun is ahead, 35° up.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::{
    Feed, FrameView, Hooks, Needs, RenderError, Renderer, WaterEffects, WorldSurface,
};
use exact_gpu::{fixture, wgpu, Gpu, Surface, Value};

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(fixture::device())
}

/// The underwater recipe's luminance band (`tests/underwater.rs`).
const BAND: std::ops::RangeInclusive<f32> = 0.13..=0.33;
const HEIGHT: f32 = 6.;
const W: u32 = 320;
const H: u32 = 180;

#[derive(Clone, Copy, Default, Args)]
struct PoolArgs {
    water: bool,
    /// The caustics' strength; negative for none.
    caustics: f32,
    shafts: bool,
    /// The camera's height, yaw and pitch (degrees; 0 looks along −Z).
    y: f32,
    yaw: f32,
    pitch: f32,
    floor: f32,
    /// An opaque slab between the sun and the floor, above the camera.
    slab: bool,
    /// A dark box crossing the view near the surface at 4 m/s.
    occluder: bool,
    /// A flat, opaque quad at the surface's height.
    flat: bool,
    /// A translucent red box straddling the surface ahead.
    glass: bool,
    /// The sun's elevation in degrees, from straight ahead.
    sun: f32,
    /// `Fog::linear(1, 15)` in place of the preset's exponential fog.
    linear: bool,
    /// Water that hides what is beneath from above: clarity 0, black.
    murk: bool,
    /// A glowing white pillar 30 m ahead, standing out of the water.
    pillar: bool,
    /// An orthographic camera this many metres tall; 0 for perspective.
    ortho: f32,
}
type Shot = PoolArgs;
struct Pool;
impl Game for Pool {
    const ID: &'static str = "water-pool";
    type Args = PoolArgs;
    fn setup(w: &mut World, a: &PoolArgs) {
        w.insert_resource(Environment {
            fog: if a.linear {
                Some(Fog::linear(1., 15.))
            } else {
                Environment::underwater().fog
            },
            ..Environment::underwater()
        });
        if a.water {
            let mut water = Water::at(HEIGHT);
            water.caustics = (a.caustics >= 0.).then_some(Caustics {
                strength: a.caustics,
                ..Caustics::default()
            });
            if !a.shafts {
                water.shafts = None;
            }
            if a.murk {
                (water.clarity, water.color) = (0., [0.; 3]);
            }
            w.insert_resource(water);
        }
        let elevation = a.sun.to_radians();
        let to_sun = Vec3::new(0., elevation.sin(), -elevation.cos());
        w.spawn_named(
            "sun",
            (
                Transform::at(to_sun.x * 30., to_sun.y * 30., to_sun.z * 30.)
                    .looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight {
                    color: [0.8, 0.95, 1.],
                    illuminance: 6000.,
                    shadows: true,
                },
            ),
        );
        let box_at = |w: &mut World, name: &str, at: Vec3, size: Vec3, material: Material| {
            w.spawn_named(
                name,
                (
                    Transform::at(at.x, at.y, at.z),
                    Mesh::cuboid(size),
                    material,
                ),
            );
        };
        w.spawn_named(
            "sand",
            (
                Transform::at(0., a.floor, 0.),
                Mesh::plane(400., 400.),
                Material::rgb(0.45, 0.38, 0.24),
            ),
        );
        let grey = |v: f32| Material::rgb(v, v, v);
        if a.slab {
            box_at(
                w,
                "slab",
                Vec3::new(0., 5.3, 0.),
                Vec3::new(60., 0.2, 60.),
                grey(0.2),
            );
        }
        if a.occluder {
            let size = Vec3::new(1.2, 0.4, 1.2);
            box_at(w, "occluder", Vec3::new(-8., 5.3, -3.), size, grey(0.1));
        }
        if a.flat {
            let size = Vec3::new(400., 0.01, 400.);
            let pale = Material::rgb(0.6, 0.9, 0.95);
            box_at(w, "flat", Vec3::new(0., HEIGHT, 0.), size, pale);
        }
        if a.pillar {
            let white = Material::glow([1., 1., 1.]);
            box_at(
                w,
                "pillar",
                Vec3::new(0., HEIGHT, -30.),
                Vec3::new(2., 8., 2.),
                white,
            );
        }
        if a.glass {
            let red = Material::rgb(1., 0.05, 0.05).alpha(0.6);
            let (at, size) = (Vec3::new(0., HEIGHT, -4.), Vec3::new(1.5, 2., 0.4));
            box_at(w, "glass", at, size, red);
        }
        let rotation = Quat::from_euler(
            glam::EulerRot::YXZ,
            a.yaw.to_radians(),
            a.pitch.to_radians(),
            0.,
        );
        let eye = Transform {
            position: Vec3::new(0., a.y, 0.),
            rotation,
            scale: Vec3::ONE,
        };
        let camera = if a.ortho > 0. {
            Camera::orthographic(a.ortho)
        } else {
            Camera::default()
        };
        w.spawn_named("camera", (eye, camera));
    }
    fn tick(w: &mut World, _: &Input, a: &PoolArgs) {
        if a.occluder {
            w.require_mut::<Transform>("occluder").position.x += 4. * w.dt();
        }
    }
}

/// From 2 m below the surface, level, towards the sun 35° up.
const BELOW: Shot = Shot {
    water: true,
    caustics: 0.45,
    shafts: true,
    y: 4.,
    yaw: 0.,
    pitch: 0.,
    floor: 0.,
    slab: false,
    occluder: false,
    flat: false,
    glass: false,
    sun: 35.,
    linear: false,
    murk: false,
    pillar: false,
    ortho: 0.,
};
impl Shot {
    fn args(self) -> Vec<Value> {
        let (b, n) = (Value::Bool, |v: f32| Value::Number(f64::from(v)));
        vec![
            b(self.water),
            n(self.caustics),
            b(self.shafts),
            n(self.y),
            n(self.yaw),
            n(self.pitch),
            n(self.floor),
            b(self.slab),
            b(self.occluder),
            b(self.flat),
            b(self.glass),
            n(self.sun),
            b(self.linear),
            b(self.murk),
            b(self.pillar),
            n(self.ortho),
        ]
    }
}

/// A look frame's picture: decoded RGBA rows.
struct Picture {
    rgba: Vec<u8>,
}
type Span = std::ops::Range<u32>;
impl Picture {
    fn linear(&self, x: u32, y: u32) -> [f32; 3] {
        let i = ((y * W + x) * 4) as usize;
        [0, 1, 2].map(|c| decode(self.rgba[i + c]))
    }
    fn luminance(&self, x: u32, y: u32) -> f32 {
        let [r, g, b] = self.linear(x, y);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }
    fn region(xs: Span, ys: Span) -> Vec<(u32, u32)> {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y))).collect()
    }
    /// Mean relative luminance, linear, as the look frame's tells measure it.
    fn mean(&self, xs: Span, ys: Span) -> f32 {
        let at = Self::region(xs, ys);
        at.iter().map(|&(x, y)| self.luminance(x, y)).sum::<f32>() / at.len() as f32
    }
    fn variance(&self, xs: Span, ys: Span) -> f32 {
        let mean = self.mean(xs.clone(), ys.clone());
        let at = Self::region(xs, ys);
        let squares = at
            .iter()
            .map(|&(x, y)| (self.luminance(x, y) - mean).powi(2));
        squares.sum::<f32>() / at.len() as f32
    }
    /// The hue of the region's mean linear colour, in degrees.
    fn hue(&self, xs: Span, ys: Span) -> f32 {
        let sum = Self::region(xs, ys).iter().fold([0f32; 3], |s, &(x, y)| {
            let c = self.linear(x, y);
            [s[0] + c[0], s[1] + c[1], s[2] + c[2]]
        });
        hue(sum)
    }
}
fn hue([r, g, b]: [f32; 3]) -> f32 {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = (max - min).max(1e-9);
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.)
    } else if max == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    };
    h * 60.
}
fn decode(v: u8) -> f32 {
    let c = f32::from(v) / 255.;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
/// RFC 4648 base64, decoded (a look picture's `data`).
fn base64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let bytes: Vec<u8> = text.bytes().filter(|&c| c != b'=').map(value).collect();
    let word =
        |c: &[u8]| (c.iter().enumerate()).fold(0u32, |n, (i, &v)| n | u32::from(v) << (18 - 6 * i));
    bytes
        .chunks(4)
        .flat_map(|c| {
            let n = word(c);
            [(n >> 16) as u8, (n >> 8) as u8, n as u8]
                .into_iter()
                .take(c.len() - 1)
        })
        .collect()
}

/// A device-less surface on the pool (the GPU-less proof host's look frames,
/// on a device of their own), its clock started at the frame's size.
fn pool(shot: Shot) -> WorldSurface<Pool> {
    let mut s = WorldSurface::<Pool>::default();
    s.bind(&shot.args(), None).unwrap();
    let start = format!(r#"{{"op":"clock","now":0,"width":{W},"height":{H}}}"#);
    s.agent(&start).unwrap();
    s
}
/// The look frame at `now` ms on the agent's clock: its picture, saved under
/// `name` with `EXACT_GPU_OUT`, and its tells.
fn look(s: &mut WorldSurface<Pool>, now: u32, name: &str) -> (Picture, serde_json::Value) {
    s.agent(&format!(r#"{{"op":"clock","now":{now}}}"#))
        .unwrap();
    let request =
        format!(r#"{{"op":"screenshot","form":"look","now":{now},"width":{W},"height":{H}}}"#);
    let reply: serde_json::Value = serde_json::from_str(&s.agent(&request).unwrap()).unwrap();
    let data = reply["data"].as_str().unwrap_or_else(|| panic!("{reply}"));
    assert_eq!(reply["width"], W, "{}", reply["look"]);
    let rgba = base64(data);
    let pixels = fixture::Pixels {
        width: W,
        height: H,
        data: rgba.clone(),
    };
    pixels.save(name);
    (Picture { rgba }, reply["look"].clone())
}
fn shoot(shot: Shot, name: &str) -> Picture {
    look(&mut pool(shot), 500, name).0
}

#[test]
fn from_below_the_sky_shows_through_snells_window_and_from_above_the_floor() {
    let Some(_gpu) = gpu() else { return };
    let bright = BAND.end() + 0.1;
    // Looking 50° up from 2 m below: the window fills the upper quarter.
    let up = Shot {
        pitch: 50.,
        ..BELOW
    };
    let (window, look) = look(&mut pool(up), 500, "water-window");
    let none = shoot(Shot { water: false, ..up }, "water-window-none");
    let flat = shoot(
        Shot {
            water: false,
            flat: true,
            ..up
        },
        "water-window-flat",
    );
    let quarter = |p: &Picture| p.mean(0..W, 0..H / 4);
    let quarters = [&window, &none, &flat].map(quarter);
    // Pitched 45°, the window's edge (48.6° from the zenith) crosses the
    // frame: the top fifth is inside it, the bottom fifth outside, reflecting.
    let steep = Shot {
        pitch: 45.,
        ..BELOW
    };
    let step = |p: &Picture| p.mean(0..W, 0..H / 5) - p.mean(0..W, H * 4 / 5..H);
    let steps = [
        shoot(steep, "water-edge"),
        shoot(
            Shot {
                water: false,
                ..steep
            },
            "water-edge-none",
        ),
        shoot(
            Shot {
                water: false,
                flat: true,
                ..steep
            },
            "water-edge-flat",
        ),
    ]
    .map(|p| step(&p));
    // From 1 m above a floor 1 m down: the floor shows through.
    let above = Shot {
        y: 7.,
        pitch: -60.,
        floor: 5.,
        ..BELOW
    };
    let lower = |p: &Picture| p.hue(0..W, H / 2..H);
    let hues = [
        shoot(above, "water-above"),
        shoot(
            Shot {
                water: false,
                ..above
            },
            "water-above-none",
        ),
        shoot(
            Shot {
                water: false,
                flat: true,
                ..above
            },
            "water-above-flat",
        ),
    ]
    .map(|p| lower(&p));
    eprintln!(
        "water surface: upper quarter {:.3} (none {:.3}, flat quad {:.3}; at least {bright:.2}); \
         window top less bottom {:.3} (none {:.3}, flat quad {:.3}); from above, lower half's hue \
         {:.1}° (the floor with no water {:.1}°, a flat quad {:.1}°); the window's look {}",
        quarters[0],
        quarters[1],
        quarters[2],
        steps[0],
        steps[1],
        steps[2],
        hues[0],
        hues[1],
        hues[2],
        look["tells"]
    );
    assert!(
        quarters[0] >= bright,
        "the sky through the window: {quarters:?}"
    );
    assert!(quarters[1] < bright, "no water: {quarters:?}");
    assert!(
        quarters[2] < bright,
        "a flat quad is no window: {quarters:?}"
    );
    assert!(
        steps[0] > 0.2,
        "inside the window against outside it: {steps:?}"
    );
    assert!(
        steps[1].abs() < 0.05 && steps[2].abs() < 0.05,
        "no edge: {steps:?}"
    );
    let off = |h: f32| (h - hues[1] + 180.).rem_euclid(360.) - 180.;
    assert!(
        off(hues[0]).abs() <= 10.,
        "the floor's hue through the surface: {hues:?}"
    );
    assert!(
        off(hues[2]).abs() > 10.,
        "a flat quad hides the floor: {hues:?}"
    );
    // The window's brightness is no tell.
    assert_eq!(look["tells"], serde_json::json!([]), "{look}");
    assert_eq!(look["gaps"], serde_json::json!([]), "{look}");
}

#[test]
fn caustics_net_the_lit_floor_and_leave_the_shadowed_floor_as_it_was() {
    let Some(_gpu) = gpu() else { return };
    let down = Shot {
        pitch: -60.,
        ..BELOW
    };
    let (xs, ys) = (W / 2 - 32..W / 2 + 32, H / 2 - 32..H / 2 + 32);
    let patch = |shot: Shot, name: &str| {
        let p = shoot(shot, name);
        (
            p.mean(xs.clone(), ys.clone()),
            p.variance(xs.clone(), ys.clone()),
        )
    };
    let lit = patch(down, "water-caustics");
    let plain = patch(
        Shot {
            caustics: -1.,
            ..down
        },
        "water-caustics-none",
    );
    let constant = patch(
        Shot {
            caustics: 0.,
            ..down
        },
        "water-caustics-constant",
    );
    // An opaque slab over the camera puts the whole floor in the sun's shadow.
    let shaded = Shot { slab: true, ..down };
    let shade = patch(shaded, "water-caustics-shade");
    let shade_plain = patch(
        Shot {
            caustics: -1.,
            ..shaded
        },
        "water-caustics-shade-none",
    );
    eprintln!(
        "caustics, a 64 x 64 patch of lit sand: variance {:.6} against {:.6} without ({:.1}x; \
         strength 0, a constant: {:.6}); in shadow, mean {:.4} against {:.4} without, \
         variance {:.7} against {:.7}",
        lit.1,
        plain.1,
        lit.1 / plain.1,
        constant.1,
        shade.0,
        shade_plain.0,
        shade.1,
        shade_plain.1
    );
    assert!(
        lit.1 >= 4. * plain.1,
        "a net on lit sand: {lit:?} {plain:?}"
    );
    assert!(
        constant.1 < 4. * plain.1,
        "a constant is no net: {constant:?}"
    );
    // Nor a pattern that keeps the mean: the shadowed patch is as even as without.
    assert!(
        shade.1 <= 1.05 * shade_plain.1 + 1e-6,
        "no net in shadow: {shade:?} {shade_plain:?}"
    );
    assert!(
        (shade.0 - shade_plain.0).abs() <= 0.05 * shade_plain.0,
        "none in shadow: {shade:?} {shade_plain:?}"
    );
}

#[test]
fn shafts_brighten_the_water_towards_the_sun_and_an_occluder_darkens_them() {
    let Some(_gpu) = gpu() else { return };
    // Pitched 20° towards the sun, and turned 40° from it; the column is 24
    // pixels wide (the streaks are narrower), from the top quarter to the middle.
    let toward = Shot {
        pitch: 20.,
        ..BELOW
    };
    let column = |p: &Picture| p.mean(W / 2 - 12..W / 2 + 12, H / 4..H / 2);
    let lit = [
        shoot(toward, "water-shafts"),
        shoot(Shot { yaw: 40., ..toward }, "water-shafts-turned"),
    ]
    .map(|p| column(&p));
    let dark = [
        shoot(
            Shot {
                shafts: false,
                ..toward
            },
            "water-shafts-none",
        ),
        shoot(
            Shot {
                yaw: 40.,
                shafts: false,
                ..toward
            },
            "water-shafts-none-turned",
        ),
    ]
    .map(|p| column(&p));
    // A dark box 1.3 m above the camera and 3 m ahead crosses the view at
    // 4 m/s: in front of the column at 2 s, 8 m to its right at 4 s. Below
    // it, the column's lower half.
    let below = |p: &Picture| p.mean(W / 2 - 12..W / 2 + 12, H / 2 + 10..H * 3 / 4 + 10);
    let crossing = |occluder: bool, name: &str| {
        let mut s = pool(Shot { occluder, ..toward });
        let ahead = look(&mut s, 2000, &format!("{name}-2s")).0;
        let past = look(&mut s, 4000, &format!("{name}-4s")).0;
        [below(&ahead), below(&past)]
    };
    let boxed = crossing(true, "water-occluder");
    let open = crossing(false, "water-open");
    eprintln!(
        "shafts, the column towards the sun less 40° from it: {:.3} ({:.3}, {:.3}); without \
         shafts {:.3}; below the column, with the box in front {:.3} against {:.3} open, \
         with it past {:.3} against {:.3}",
        lit[0] - lit[1],
        lit[0],
        lit[1],
        dark[0] - dark[1],
        boxed[0],
        open[0],
        boxed[1],
        open[1]
    );
    assert!(lit[0] - lit[1] >= 0.05, "towards the sun: {lit:?}");
    assert!((dark[0] - dark[1]).abs() <= 0.01, "no shafts: {dark:?}");
    assert!(
        boxed[0] < open[0] - 0.02,
        "the box shades its shaft: {boxed:?} {open:?}"
    );
    assert!(
        (boxed[1] - open[1]).abs() < 0.005,
        "past it, none: {boxed:?} {open:?}"
    );
}

#[test]
fn the_surface_hides_what_is_above_it_and_translucency_below_draws_over_it() {
    let Some(_gpu) = gpu() else { return };
    // A translucent red box straddles the surface 4 m ahead; from 2 m below,
    // pitched 30°, its lower half is under the surface and its upper half over it.
    let up = Shot {
        pitch: 30.,
        shafts: false,
        ..BELOW
    };
    let glass = shoot(Shot { glass: true, ..up }, "water-glass");
    let clear = shoot(up, "water-glass-none");
    // The rows where the box meets the surface, from the camera at 4 m, 30° up.
    let row = |y: f32| {
        let angle = ((y - 4.) / 4.).atan() - 30f32.to_radians();
        (H as f32 / 2. * (1. - angle.tan() / 30f32.to_radians().tan())) as u32
    };
    let (under, over) = (row(5.5), row(6.6));
    let red = |p: &Picture, y: u32| {
        let [r, g, b] = p.linear(W / 2, y);
        r - (g + b) / 2.
    };
    eprintln!(
        "glass across the surface: redness under it {:.3} (clear {:.3}), over it {:.3} \
         (clear {:.3}), rows {under} and {over}",
        red(&glass, under),
        red(&clear, under),
        red(&glass, over),
        red(&clear, over)
    );
    assert!(red(&glass, under) > red(&clear, under) + 0.05);
    // Over the surface the glass is hidden: the frame is the clear one's, within
    // what another GPU's bloom may move.
    let (a, b) = (glass.linear(W / 2, over), clear.linear(W / 2, over));
    assert!((0..3).all(|c| (a[c] - b[c]).abs() < 0.01), "{a:?} {b:?}");
}

/// The pool hostless, drawn by a renderer of its own with `effects`.
fn draw(gpu: &Gpu, sim: &Sim<Pool>, effects: WaterEffects, name: &str) -> fixture::Pixels {
    draw_with(gpu, sim, effects, SceneSamples::Four, name)
}
fn draw_with(
    gpu: &Gpu,
    sim: &Sim<Pool>,
    effects: WaterEffects,
    samples: SceneSamples,
    name: &str,
) -> fixture::Pixels {
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::with_samples(&gpu.device, &gpu.queue, format, samples);
    renderer.set_water_effects(effects);
    let mut feed = Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("water"),
        size: wgpu::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let frame = feed.frame(sim.world(), 1., W as f32 / H as f32);
    renderer.draw(&texture.create_view(&Default::default()), (W, H), &frame);
    let pixels = fixture::read(gpu, &texture).unwrap();
    pixels.save(name);
    pixels
}

#[test]
fn clearing_an_effect_at_present_time_draws_the_frame_without_it_and_keeps_the_resource() {
    let Some(gpu) = gpu() else { return };
    let toward = Shot {
        pitch: 20.,
        ..BELOW
    };
    let run = |shot: Shot| {
        let mut sim = Sim::<Pool>::new(shot).unwrap();
        sim.run(1500.);
        sim
    };
    let full = run(toward);
    let without = run(Shot {
        caustics: -1.,
        shafts: false,
        ..toward
    });
    let neither = WaterEffects {
        caustics: false,
        shafts: false,
    };
    let all = draw(&gpu, &full, WaterEffects::default(), "water-effects-all");
    let cleared = draw(&gpu, &full, neither, "water-effects-cleared");
    let absent = draw(
        &gpu,
        &without,
        WaterEffects::default(),
        "water-effects-absent",
    );
    assert!(all.data != cleared.data, "the effects drew");
    assert!(cleared.data == absent.data, "cleared is as if absent");
    let water = *full.world().resource::<Water>();
    assert!(water.caustics.is_some() && water.shafts.is_some());
    // At one sample a pixel (which would otherwise draw direct) the same water
    // draws, within a band of the four-sample frame.
    let one = draw_with(
        &gpu,
        &full,
        WaterEffects::default(),
        SceneSamples::One,
        "water-one",
    );
    let mean = |p: &fixture::Pixels| {
        let sum: f32 = p
            .data
            .chunks_exact(4)
            .map(|c| f32::from(c[0]) + f32::from(c[1]) + f32::from(c[2]))
            .sum();
        sum / (p.data.len() as f32 * 0.75 * 255.)
    };
    let (four, single) = (mean(&all), mean(&one));
    assert!(
        (four - single).abs() < 0.01,
        "four samples {four}, one {single}"
    );
}

#[test]
fn a_restored_world_draws_the_same_water_and_its_waves_move_on() {
    let Some(gpu) = gpu() else { return };
    // A still scene: nothing moves but the water.
    let toward = Shot {
        pitch: 20.,
        ..BELOW
    };
    let mut first = Sim::<Pool>::new(toward).unwrap();
    // Fields away from their defaults, which only the save carries.
    *first.world_mut().resource_mut::<Water>() = Water {
        clarity: 0.7,
        waves: Waves {
            amplitude: 0.05,
            wavelength: 1.3,
            speed: 0.9,
        },
        caustics: Some(Caustics {
            strength: 0.3,
            scale: 2.2,
        }),
        shafts: Some(Shafts {
            intensity: 0.9,
            length: 0.4,
        }),
        ..Water::at(5.5)
    };
    first.run(1500.);
    let saved = first.save().unwrap();
    let mut second = Sim::<Pool>::new(toward).unwrap();
    second.restore(&saved).unwrap();
    assert_eq!(second.world().hash(), first.world().hash());
    assert_eq!(
        *second.world().resource::<Water>(),
        *first.world().resource::<Water>()
    );
    let effects = WaterEffects::default();
    let a = draw(&gpu, &first, effects, "water-saved");
    let b = draw(&gpu, &second, effects, "water-restored");
    assert!(a.data == b.data, "the restored frame differs");
    // Both run on: the same frame again, and another than 100 ms before, which
    // only the waves, the caustics and the shafts' drift can make.
    first.run(100.);
    second.run(100.);
    let c = draw(&gpu, &first, effects, "water-later");
    let d = draw(&gpu, &second, effects, "water-restored-later");
    assert!(c.data == d.data, "the restored world ran on differently");
    let changed = a.data.iter().zip(&c.data).filter(|(x, y)| x != y).count();
    eprintln!(
        "water 100 ms on: {changed} bytes of {} changed",
        a.data.len()
    );
    assert!(
        changed > a.data.len() / 20,
        "the water stood still: {changed}"
    );
}

/// A hook that asks for the scene copy and nothing after it.
#[derive(Default)]
struct CopyHook;
impl Hooks for CopyHook {
    fn needs(&self) -> Needs {
        Needs::SCENE_COPY
    }
}
/// A background hook that leaves a quarter-size viewport and scissor behind.
#[derive(Default)]
struct CornerHook;
impl Hooks for CornerHook {
    fn background(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        view: &FrameView<'_>,
    ) -> Result<(), RenderError> {
        let (w, h) = (view.pixels.0 / 4, view.pixels.1 / 4);
        pass.set_viewport(0., 0., w as f32, h as f32, 0., 1.);
        pass.set_scissor_rect(0, 0, w, h);
        Ok(())
    }
}
/// The shot on a canvas that draws, through `H`, at 500 ms.
fn drawn<H: Hooks>(gpu: &Gpu, shot: Shot, name: &str) -> fixture::Pixels {
    let mut s = WorldSurface::<Pool, (), false, H>::default();
    s.bind(&shot.args(), None).unwrap();
    let frame = |now_ms| exact_gpu::Frame {
        width: W as f32,
        height: H as f32,
        scale: 1.,
        now_ms,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.,
    };
    let mut pixels = None;
    for at in [0., 500.] {
        pixels = Some(fixture::render(gpu, &mut s, &frame(at)).unwrap().0);
        assert!(s.error().is_none(), "{:?}", s.error());
    }
    let pixels = pixels.unwrap();
    pixels.save(name);
    pixels
}

#[test]
fn a_hook_s_scene_copy_or_background_leaves_the_water_whole() {
    let Some(gpu) = gpu() else { return };
    let toward = Shot {
        pitch: 20.,
        ..BELOW
    };
    let plain = drawn::<()>(&gpu, toward, "water-hook-none");
    let copy = drawn::<CopyHook>(&gpu, toward, "water-hook-copy");
    let corner = drawn::<CornerHook>(&gpu, toward, "water-hook-corner");
    let differ = |a: &fixture::Pixels, b: &fixture::Pixels| {
        let n = a
            .data
            .iter()
            .zip(&b.data)
            .filter(|(x, y)| x.abs_diff(**y) > 2)
            .count();
        n as f32 / a.data.len() as f32
    };
    let (copied, cornered) = (differ(&plain, &copy), differ(&plain, &corner));
    eprintln!(
        "water under hooks: {:.4} of bytes differ under a scene copy, {:.4} after a background \
         hook's viewport",
        copied, cornered
    );
    // The scene copy keeps the depth the shafts read; the surface draws whole.
    assert!(copied < 0.01, "scene copy: {copied}");
    assert!(cornered < 0.01, "background viewport: {cornered}");
}

#[test]
fn no_shafts_from_a_sun_below_the_horizon() {
    let Some(_gpu) = gpu() else { return };
    let under = Shot {
        pitch: 20.,
        sun: -20.,
        ..BELOW
    };
    let shafts = shoot(under, "water-sunset");
    let none = shoot(
        Shot {
            shafts: false,
            ..under
        },
        "water-sunset-none",
    );
    let differ = shafts
        .rgba
        .iter()
        .zip(&none.rgba)
        .filter(|(a, b)| a != b)
        .count();
    eprintln!("a sun 20° below the horizon: {differ} bytes differ with shafts");
    assert_eq!(differ, 0, "shafts from a sun below the horizon");
}

#[test]
fn an_orthographic_camera_meets_the_water_at_each_ray() {
    let Some(_gpu) = gpu() else { return };
    // Looking up 46° from 2 m under, 20 m tall: every parallel ray is near the
    // window's edge, so the waves turn parts of the surface into mirrors.
    let up = Shot {
        pitch: 44.,
        ortho: 20.,
        shafts: false,
        ..BELOW
    };
    let p = shoot(up, "water-ortho-up");
    // The lower rays start under the surface (the upper ones on the camera's
    // plane are above it and see the sky).
    let surface = p.variance(0..W, H * 5 / 8..H);
    // The plan view, from above: the floor's caustics vary across it.
    let mut s = pool(BELOW);
    s.agent(r#"{"op":"clock","now":500}"#).unwrap();
    let request = format!(
        r#"{{"op":"screenshot","form":"look","now":500,"width":{W},"height":{H},"top":30}}"#
    );
    let reply: serde_json::Value = serde_json::from_str(&s.agent(&request).unwrap()).unwrap();
    let plan = Picture {
        rgba: base64(reply["data"].as_str().unwrap()),
    };
    fixture::Pixels {
        width: W,
        height: H,
        data: plan.rgba.clone(),
    }
    .save("water-ortho-plan");
    let floor = plan.variance(W / 4..W * 3 / 4, H / 4..H * 3 / 4);
    eprintln!(
        "orthographic: the surface from below, variance {surface:.6}; the plan's floor {floor:.6}"
    );
    assert!(
        surface > 1e-4,
        "one hit for every ray paints a flat tint: {surface}"
    );
    assert!(floor > 5e-5, "the plan's floor is flat: {floor}");
}

#[test]
fn linear_fog_under_water_scatters_shafts_and_stops_at_the_surface() {
    let Some(_gpu) = gpu() else { return };
    // From 1 m over the water, level: a white pillar 30 m off stands out of it.
    let above = Shot {
        y: 7.,
        floor: 5.,
        linear: true,
        pillar: true,
        shafts: false,
        ..BELOW
    };
    // The pillar's row 3 m over the surface, 30 m off.
    let row = (H as f32 / 2. * (1. - (3f32 / 30.).atan().tan() / 30f32.to_radians().tan())) as u32;
    let pillar = |p: &Picture| p.mean(W / 2 - 2..W / 2 + 2, row - 1..row + 2);
    let dry = pillar(&shoot(above, "water-linear-pillar"));
    let fogged = pillar(&shoot(
        Shot {
            water: false,
            ..above
        },
        "water-linear-pillar-none",
    ));
    // From below, towards the sun: shafts scatter in the linear fog.
    let toward = Shot {
        pitch: 20.,
        linear: true,
        ..BELOW
    };
    let column = |p: &Picture| p.mean(W / 2 - 12..W / 2 + 12, H / 4..H / 2);
    let lit = column(&shoot(toward, "water-linear-shafts"));
    let dark = column(&shoot(
        Shot {
            shafts: false,
            ..toward
        },
        "water-linear-shafts-none",
    ));
    eprintln!(
        "linear fog: the pillar over the water {dry:.3} (fogged without water {fogged:.3}); \
         the column towards the sun {lit:.3} with shafts, {dark:.3} without"
    );
    assert!(dry > fogged + 0.2, "linear fog stops at the surface");
    assert!(lit > dark + 0.02, "shafts scatter in linear fog");
}

#[test]
fn from_above_murky_water_reflects_more_at_a_grazing_angle() {
    let Some(_gpu) = gpu() else { return };
    // From 1 m over water that hides the floor and is black itself, pitched
    // 15° down: what shows is the sky's reflection, by Fresnel.
    let above = Shot {
        y: 7.,
        pitch: -15.,
        floor: 5.,
        murk: true,
        ..BELOW
    };
    let murk = shoot(above, "water-murk");
    let clear = shoot(
        Shot {
            murk: false,
            ..above
        },
        "water-murk-clear",
    );
    // The horizon is at row 48; just below it the view grazes the surface.
    let (grazing, steep) = (H / 4 + 8..H / 4 + 16, H - 20..H);
    let band = |p: &Picture, rows: Span| p.mean(0..W, rows);
    eprintln!(
        "from above, murky: grazing {:.3}, steep {:.3}; clear water's steep band {:.3}",
        band(&murk, grazing.clone()),
        band(&murk, steep.clone()),
        band(&clear, steep.clone())
    );
    assert!(
        band(&murk, steep.clone()) < 0.03,
        "steep: about 2% reflected"
    );
    assert!(
        band(&murk, grazing) > band(&murk, steep.clone()) + 0.03,
        "grazing: more reflected"
    );
    assert!(band(&clear, steep) > 0.1, "clear water shows the floor");
}
