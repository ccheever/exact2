use exact_game::character::Character;
use exact_game::*;

#[derive(Default, exact_game::Data)]
struct Hud {
    lit: u32,
    near: String,
}
#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    // Restart idiom: changing restart_generation reconstructs setup; Play again increments it.
    pub restart_generation: u32,
}
#[derive(Default, Component)]
struct Player {
    character: Character,
}
#[derive(Default, Component)]
pub struct Beacon {
    pub lit: bool,
    glow: Spring,
}
#[derive(Kind)]
struct Hero {
    player: Player,
    transform: Transform,
}
#[derive(Kind)]
struct Lightable {
    #[read]
    transform: Transform,
    beacon: Beacon,
}
#[derive(Kind)]
struct Glow {
    #[read]
    beacon: Beacon,
    material: Material,
}
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows())
            .button("light", &["KeyE"])
            .button("jump", &["Space"])
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.spawn((
            Transform::default(),
            Mesh::plane(40.0, 40.0),
            Material::grid([0.16, 0.23, 0.24], 1.0),
        ));
        let player = w.spawn_kind(
            "player",
            Hero {
                transform: Transform::at(0.0, 0.9, 0.0),
                player: Player {
                    character: Character::new().ground(0.9).bounds_xz(-19.6..=19.6),
                },
            },
        );
        w.insert(player.entity(), Mesh::capsule(0.4, 1.8));
        w.insert(player.entity(), Material::rgb(0.8, 0.4, 0.1));
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                Follow::new(player.entity())
                    .offset(0.0, 9.0, 13.0)
                    .lag(0.15),
            ),
        );
        for (i, x) in [2.0, 6.0].into_iter().enumerate() {
            w.spawn_named(
                format!("plinth-{}", i + 1),
                (
                    Transform::at(x, 0.1, 0.0),
                    Mesh::cylinder(0.9, 0.2),
                    Material::rgb(0.3, 0.4, 0.42),
                ),
            );
            let beacon = w.spawn_kind(
                format!("beacon-{}", i + 1),
                Lightable {
                    transform: Transform::at(x, 0.7, 0.0),
                    beacon: Beacon::default(),
                },
            );
            w.insert(beacon.entity(), Mesh::sphere(0.5));
            w.insert(beacon.entity(), Material::default());
        }
        w.publish_record(&Hud::default());
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        let dt = w.dt();
        let position = w.edit(w.the::<Hero>(), |h| {
            h.player.character.step(
                &mut h.transform,
                input.stick_xz("move"),
                input.pressed("jump"),
                dt,
            );
            h.transform.position
        });
        let mut nearest = None;
        let mut selected = Vec::new();
        for row in w.rows::<Lightable>() {
            let d = row.transform.position - position;
            let distance = Vec2::new(d.x, d.z).length_squared();
            if distance > 2.25 || row.beacon.lit {
                continue;
            }
            if input.pressed("light") {
                selected.push(row.id);
            } else if nearest.is_none_or(|(_, old)| distance < old) {
                nearest = Some((row.id.entity(), distance));
            }
        }
        for id in selected {
            w.edit(id, |row| {
                row.beacon.lit = true;
                row.beacon.glow.set_target(w.now(), 1.0);
            });
        }
        w.publish("near", nearest.and_then(|(e, _)| w.name(e)).unwrap_or(""));
        let near = nearest
            .and_then(|(e, _)| w.name(e))
            .unwrap_or("")
            .to_owned();
        let mut count = 0;
        for mut row in w.rows_mut::<Glow>() {
            row.material.emissive = [row.beacon.glow.value(w.now()) * 3.0; 3];
            count += u32::from(row.beacon.lit);
        }
        w.publish_record(&Hud { lit: count, near });
        scene::follow(w);
    }
}
