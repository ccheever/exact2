//! First-person weapon feedback, driven by saved world time without gameplay RNG.
use crate::{
    fighter::Fighter,
    weapons::{Damage, Weapon},
};
use exact_game::{
    audio::{self, AudioListener, Spatial, Synth},
    *,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub enum PartRole {
    #[default]
    Body,
    Magazine,
    Flash,
}

#[derive(Clone, Debug, Default, Component)]
pub struct WeaponPart {
    pub weapon: Weapon,
    pub rest: Vec3,
    pub rotation: Quat,
    pub role: PartRole,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct Feedback {
    pub shot_tick: Option<u64>,
    pub weapon: Weapon,
}

struct Model<'a> {
    w: &'a mut World,
    camera: Entity,
    weapon: Weapon,
}
impl Model<'_> {
    fn part(
        &mut self,
        name: &str,
        at: Vec3,
        mesh: Mesh,
        material: Material,
        rotation: Quat,
        role: PartRole,
    ) {
        self.w.spawn_named(
            name,
            (
                Parent(self.camera),
                Transform {
                    position: at,
                    rotation,
                    ..Transform::default()
                },
                mesh,
                material,
                Visible(self.weapon == Weapon::Rifle && role != PartRole::Flash),
                ViewModel,
                WeaponPart {
                    weapon: self.weapon,
                    rest: at,
                    rotation,
                    role,
                },
            ),
        );
    }
    fn block(&mut self, name: &str, at: [f32; 3], size: [f32; 3], material: Material) {
        self.part(
            name,
            Vec3::from_array(at),
            Mesh::cuboid(Vec3::from_array(size)),
            material,
            Quat::IDENTITY,
            PartRole::Body,
        );
    }
    fn tube(&mut self, name: &str, at: [f32; 3], radius: f32, length: f32, material: Material) {
        self.part(
            name,
            Vec3::from_array(at),
            Mesh::cylinder(radius, length),
            material,
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            PartRole::Body,
        );
    }
    fn flash(&mut self, prefix: &str, at: [f32; 3]) {
        for (suffix, rotation) in [("a", 0.7), ("b", -0.7)] {
            self.part(
                &format!("{prefix}-{suffix}"),
                Vec3::from_array(at),
                Mesh::cuboid(Vec3::new(0.025, 0.13, 0.055)),
                Material::glow([8., 4., 0.5]),
                Quat::from_rotation_z(rotation),
                PartRole::Flash,
            );
        }
        self.part(
            &format!("{prefix}-core"),
            Vec3::from_array(at),
            Mesh::sphere(0.03),
            Material::glow([12., 9., 3.]),
            Quat::IDENTITY,
            PartRole::Flash,
        );
    }
}

pub fn register(w: &mut World) {
    w.register::<WeaponPart>();
    w.register_resource::<Feedback>();
}
/// The listener, the sounds and, unless the art pass draws its own (`art::dress`),
/// the classic first-person weapons.
pub fn setup(w: &mut World, camera: Entity, weapons: bool) {
    w.insert_resource(Feedback::default());
    w.insert(camera, AudioListener);
    sounds(w);
    if weapons {
        viewmodel(w, camera);
    }
}

fn viewmodel(w: &mut World, camera: Entity) {
    let steel = Material::rgb(0.16, 0.19, 0.22).metallic(0.65).rough(0.35);
    let dark = Material::rgb(0.065, 0.075, 0.085).rough(0.65);
    let tan = Material::rgb(0.47, 0.32, 0.17).rough(0.8);
    let glove = Material::rgb(0.19, 0.24, 0.22).rough(0.85);
    let sleeve = Material::rgb(0.22, 0.33, 0.43).rough(0.9);
    let mut m = Model {
        w,
        camera,
        weapon: Weapon::Rifle,
    };
    m.block("vm-rifle", [0.2, -0.19, -0.52], [0.085, 0.095, 0.30], steel);
    m.block(
        "vm-rifle-stock",
        [0.2, -0.22, -0.27],
        [0.075, 0.13, 0.20],
        tan,
    );
    m.block(
        "vm-rifle-grip",
        [0.2, -0.285, -0.44],
        [0.06, 0.14, 0.065],
        dark,
    );
    m.block(
        "vm-rifle-guard",
        [0.2, -0.18, -0.72],
        [0.075, 0.085, 0.18],
        tan,
    );
    m.tube("vm-rifle-barrel", [0.2, -0.17, -0.9], 0.022, 0.25, steel);
    m.tube("vm-rifle-muzzle", [0.2, -0.17, -1.015], 0.030, 0.065, dark);
    m.block(
        "vm-rifle-rail",
        [0.2, -0.135, -0.56],
        [0.045, 0.018, 0.23],
        dark,
    );
    for (name, z) in [("rear", -0.42), ("front", -0.86)] {
        m.block(
            &format!("vm-rifle-sight-{name}"),
            [0.2, -0.107, z],
            [0.016, 0.04, 0.025],
            steel,
        );
    }
    m.part(
        "vm-rifle-mag",
        Vec3::new(0.2, -0.305, -0.61),
        Mesh::cuboid(Vec3::new(0.060, 0.17, 0.085)),
        dark,
        Quat::from_rotation_x(-0.12),
        PartRole::Magazine,
    );
    m.block(
        "vm-rifle-hand",
        [0.22, -0.29, -0.40],
        [0.085, 0.095, 0.10],
        glove,
    );
    m.block(
        "vm-rifle-arm",
        [0.28, -0.365, -0.24],
        [0.11, 0.14, 0.28],
        sleeve,
    );
    m.block(
        "vm-rifle-support",
        [0.16, -0.24, -0.70],
        [0.09, 0.085, 0.12],
        glove,
    );
    m.block(
        "vm-rifle-support-arm",
        [0.10, -0.32, -0.56],
        [0.10, 0.14, 0.22],
        sleeve,
    );
    m.flash("vm-rifle-flash", [0.2, -0.17, -1.065]);

    m.weapon = Weapon::Rocket;
    let green = Material::rgb(0.27, 0.39, 0.22).rough(0.7);
    m.tube("vm-rocket", [0.22, -0.20, -0.48], 0.085, 0.76, green);
    m.tube("vm-rocket-front", [0.22, -0.20, -0.86], 0.105, 0.065, dark);
    m.tube("vm-rocket-back", [0.22, -0.20, -0.14], 0.095, 0.085, steel);
    for (name, z) in [("front", -0.70), ("rear", -0.31)] {
        m.tube(
            &format!("vm-rocket-band-{name}"),
            [0.22, -0.20, z],
            0.092,
            0.03,
            tan,
        );
    }
    m.block(
        "vm-rocket-sight",
        [0.22, -0.078, -0.61],
        [0.028, 0.07, 0.08],
        steel,
    );
    m.block(
        "vm-rocket-grip",
        [0.22, -0.33, -0.39],
        [0.065, 0.15, 0.085],
        dark,
    );
    m.block(
        "vm-rocket-hand",
        [0.24, -0.34, -0.36],
        [0.09, 0.1, 0.12],
        glove,
    );
    m.block(
        "vm-rocket-arm",
        [0.30, -0.40, -0.20],
        [0.12, 0.15, 0.28],
        sleeve,
    );
    m.flash("vm-rocket-flash", [0.22, -0.20, -0.93]);

    m.weapon = Weapon::Knife;
    m.block(
        "vm-knife",
        [0.22, -0.20, -0.48],
        [0.018, 0.045, 0.28],
        Material::rgb(0.72, 0.78, 0.82).metallic(0.9).rough(0.2),
    );
    m.part(
        "vm-knife-tip",
        Vec3::new(0.22, -0.2, -0.628),
        Mesh::cuboid(Vec3::new(0.018, 0.032, 0.032)),
        steel,
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_4),
        PartRole::Body,
    );
    m.block(
        "vm-knife-guard",
        [0.22, -0.20, -0.32],
        [0.04, 0.10, 0.025],
        steel,
    );
    m.block(
        "vm-knife-grip",
        [0.22, -0.20, -0.23],
        [0.038, 0.06, 0.16],
        dark,
    );
    m.block(
        "vm-knife-hand",
        [0.24, -0.24, -0.22],
        [0.09, 0.10, 0.12],
        glove,
    );
    m.block(
        "vm-knife-arm",
        [0.29, -0.34, -0.11],
        [0.12, 0.17, 0.22],
        sleeve,
    );
}

fn sounds(w: &mut World) {
    w.sounds([
        (
            "rifle-shot",
            Synth::noise()
                .seconds(0.13)
                .decay(0.08)
                .highpass_hz(700.)
                .lowpass_hz(7000.)
                .gain(0.22)
                .layer(
                    Synth::sine(135.)
                        .seconds(0.10)
                        .slide(-18.)
                        .decay(0.07)
                        .gain(0.20),
                ),
        ),
        (
            "rocket-shot",
            Synth::noise()
                .seconds(0.32)
                .decay(0.22)
                .lowpass_hz(2200.)
                .gain(0.22)
                .layer(Synth::sine(100.).seconds(0.25).slide(-18.).gain(0.18)),
        ),
        (
            "knife-swing",
            Synth::noise()
                .seconds(0.16)
                .attack(0.02)
                .decay(0.10)
                .highpass_hz(1800.)
                .lowpass_hz(4800.)
                .gain(0.10),
        ),
        (
            "hit",
            Synth::triangle(950.)
                .seconds(0.075)
                .decay(0.055)
                .slide(-10.)
                .gain(0.13),
        ),
        (
            "head-hit",
            Synth::sine(1450.)
                .seconds(0.12)
                .decay(0.09)
                .slide(3.)
                .gain(0.17),
        ),
        (
            "elimination",
            Synth::triangle(620.)
                .seconds(0.25)
                .decay(0.20)
                .slide(9.)
                .gain(0.18)
                .layer(Synth::sine(930.).seconds(0.30).decay(0.24).gain(0.09)),
        ),
        (
            "reload-out",
            Synth::noise()
                .seconds(0.11)
                .decay(0.075)
                .highpass_hz(900.)
                .lowpass_hz(3300.)
                .gain(0.12),
        ),
        (
            "reload-in",
            Synth::noise()
                .seconds(0.13)
                .decay(0.08)
                .highpass_hz(500.)
                .lowpass_hz(2400.)
                .gain(0.14)
                .layer(Synth::triangle(290.).seconds(0.075).decay(0.05).gain(0.12)),
        ),
        (
            "explosion",
            Synth::noise()
                .seconds(0.60)
                .decay(0.45)
                .lowpass_hz(1600.)
                .gain(0.26)
                .layer(
                    Synth::sine(85.)
                        .seconds(0.45)
                        .slide(-18.)
                        .decay(0.35)
                        .gain(0.24),
                ),
        ),
    ]);
}

/// Only the few fields needed to recognize successful actions, before and after.
pub struct ActionState {
    shots: u32,
    reloading: bool,
    ammo: u32,
}
impl ActionState {
    pub fn read(w: &World, entity: Entity) -> Self {
        let f = w.require::<Fighter>(entity);
        Self {
            shots: f.shots,
            reloading: f.reload_until > 0.,
            ammo: f.rifle_ammo + f.rocket_ammo,
        }
    }
}
pub fn actions(w: &World, entity: Entity, before: ActionState) {
    let f = w.require::<Fighter>(entity);
    if f.shots > before.shots {
        let sound = match f.weapon {
            Weapon::Rifle => "rifle-shot",
            Weapon::Rocket => "rocket-shot",
            Weapon::Knife => "knife-swing",
        };
        if f.bot {
            w.play(sound)
                .at(entity)
                .spatial(Spatial {
                    ref_distance: 6.,
                    ..Spatial::default()
                })
                .gain(0.6)
                .start();
        } else {
            w.play(sound).ui().start();
            *w.resource_mut::<Feedback>() = Feedback {
                shot_tick: Some(w.tick()),
                weapon: f.weapon,
            };
        }
    }
    if !f.bot {
        if !before.reloading && f.reload_until > 0. {
            w.play("reload-out").ui().start();
        } else if before.reloading
            && f.reload_until == 0.
            && f.rifle_ammo + f.rocket_ammo > before.ammo
        {
            w.play("reload-in").ui().start();
        }
    }
}
pub fn hits(w: &World, hits: &[Damage]) {
    for hit in hits.iter().filter(|h| h.attacker == 1 && h.victim != 1) {
        w.play(if hit.killed {
            "elimination"
        } else if hit.head {
            "head-hit"
        } else {
            "hit"
        })
        .ui()
        .start();
    }
}
pub fn explosion(w: &World, at: Vec3) {
    w.play("explosion")
        .at_point(at)
        .spatial(Spatial {
            ref_distance: 8.,
            ..Spatial::default()
        })
        .start();
}

pub fn pose(w: &World) {
    let now = w.seconds() as f32;
    let f = w.require::<Fighter>("player");
    let feedback = w.resource::<Feedback>();
    let age = feedback
        .shot_tick
        .map(|tick| (w.tick().saturating_sub(tick)) as f32 / w.hz() as f32);
    let flash = age.is_some_and(|age| age < 0.045) && feedback.weapon == f.weapon;
    let progress = if f.reload_until > 0. {
        1. - ((f.reload_until - now) / f.weapon.reload_time(&crate::tables::of(w))).clamp(0., 1.)
    } else {
        0.
    };
    let reload = math::sin(progress * std::f32::consts::PI);
    let tilt = Quat::from_rotation_z(-0.35 * reload);
    let pivot = Vec3::new(0.2, -0.19, -0.48);
    let swing = (now - f.swing_at).clamp(0., 0.3) / 0.3;
    for (_, (vm, t, visible)) in w
        .query::<(&WeaponPart, &mut Transform, &mut Visible)>()
        .iter()
    {
        let show = f.alive && vm.weapon == f.weapon && (vm.role != PartRole::Flash || flash);
        if visible.0 != show {
            visible.0 = show;
        }
        let mut at =
            pivot + tilt * (vm.rest - pivot) + Vec3::new(0., -0.10 * reload, 0.06 * f.kick);
        if f.aiming {
            at += Vec3::new(-0.2, 0.06, 0.);
        }
        if f.bandage_until > 0. {
            at.y -= 0.25;
        }
        if vm.role == PartRole::Magazine {
            at.y -= 0.19 * reload;
        }
        if vm.weapon == Weapon::Knife && swing < 1. {
            at += Vec3::new(-0.15, 0.05, -0.15) * math::sin(swing * std::f32::consts::PI);
        }
        let scale = if vm.role == PartRole::Flash {
            Vec3::splat(1. - 0.5 * (age.unwrap_or(0.) / 0.045).clamp(0., 1.))
        } else {
            Vec3::ONE
        };
        let next = Transform {
            position: at,
            rotation: tilt * vm.rotation,
            scale,
        };
        if *t != next {
            *t = next;
        }
    }
}
pub fn step(w: &mut World) {
    pose(w);
    audio::step(w);
}
