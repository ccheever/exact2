use exact_game::character::Character;
use exact_game::collect::{drift, Collectable};
use exact_game::spawn::Spawner;
use exact_game::*;

#[derive(Default, Args)]
pub struct Options {
    pub seed: u64,
    #[live]
    pub paused: bool,
    #[restart]
    pub restart: bool,
}
#[derive(Default, Component)]
pub struct Beacon {
    pub lit: bool,
}
/// The critter sleeps until the player comes near, then follows the player,
/// runs from lit beacons and drops coins, and starves unless fed.
#[derive(Default, Component)]
pub struct Critter {
    pub awake: bool,
    pub starved: bool,
    /// The tick it last ate (or woke); hunger is the time since, read by `hunger`.
    pub fed_at: u64,
    /// Its turning rate while it wanders, for `steer::wander`.
    pub turn: f32,
    /// When its next coin is owed, and that coin's serial.
    pub drops: Spawner,
    /// Serials of coins owed but not yet dropped: at most five lie on the ground.
    pub owed: Vec<u32>,
}
impl Critter {
    /// From 0, just fed, to 1, starved.
    pub fn hunger(&self, tick: u64) -> f32 {
        (tick.saturating_sub(self.fed_at) as f32 / (STARVES_AFTER_S * SmallGame::HZ) as f32)
            .min(1.0)
    }
}
/// The shape of this game's saves. `Game::restorable` refuses a save of another
/// version by name, so a save from before a change fails at restore, not on a
/// later tick.
#[derive(Default, Resource)]
pub struct SaveContract {
    pub version: u32,
}
/// Bump when a save from the previous version cannot run in this one. Version 2
/// adds the critter, its coins and the shop.
pub const SAVE_CONTRACT: u32 = 2;
/// What the player owns: coins and the shoes' level.
#[derive(Default, Resource)]
pub struct Shop {
    pub coins: u32,
    /// Every coin the player has picked up, spent or not.
    pub earned: u32,
    pub shoes: u32,
}
/// One shop row as the HUD shows it.
#[derive(Clone, Debug, Default, PartialEq, Data)]
pub struct ShopRow {
    pub key: String,
    pub name: String,
    pub price: u32,
    pub action: String,
    /// Why it cannot be bought now; empty when it can.
    pub reason: String,
}
#[derive(Default, Data)]
struct Hud {
    coins: u32,
    critter: String,
    shop: Vec<ShopRow>,
}

/// Seconds from fed to starved; a critter can eat once it is half hungry.
pub const STARVES_AFTER_S: u32 = 40;
/// Seconds between coins while the critter is awake and fed.
pub const COIN_EVERY_S: u32 = 2;
pub const FEED_PRICE: u32 = 6;
/// Each pair of shoes costs this more than the last, up to three.
pub const SHOES_PRICE: u32 = 8;
/// Shop rows in key order: key, name and the `Actions` button that buys it.
pub const ITEMS: [(&str, &str, &str); 2] = [("1", "Feed", "feed"), ("2", "Shoes", "shoes")];

/// Item `i`'s price, or why it is locked.
fn price(w: &World, i: usize) -> Result<u32, &'static str> {
    let shop = w.resource::<Shop>();
    let critter = w.get::<Critter>("critter");
    match i {
        0 => match critter {
            None => Err("No critter"),
            Some(c) if !c.awake => Err("Asleep"),
            Some(c) if c.starved => Err("Starved"),
            Some(c) if c.hunger(w.tick()) < 0.5 => Err("Not hungry"),
            Some(_) => Ok(FEED_PRICE),
        },
        _ if shop.shoes == 3 => Err("Max level"),
        _ => Ok(SHOES_PRICE * (shop.shoes + 1)),
    }
}
/// The shop as the HUD shows it.
pub fn shop_rows(w: &World) -> Vec<ShopRow> {
    let coins = w.resource::<Shop>().coins;
    ITEMS
        .iter()
        .enumerate()
        .map(|(i, &(key, name, action))| {
            let (price, reason) = match price(w, i) {
                Ok(p) if p > coins => (p, format!("Need {p}")),
                Ok(p) => (p, String::new()),
                Err(lock) => (0, lock.to_owned()),
            };
            ShopRow {
                key: key.into(),
                name: name.into(),
                price,
                action: action.into(),
                reason,
            }
        })
        .collect()
}
pub struct SmallGame;
impl Game for SmallGame {
    const ID: &'static str = "small-game";
    const HZ: u32 = 120;
    type Args = Options;
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_arrows().or_pad(Pad::Left))
            .button("light", &["KeyE", "GamepadWest"])
            .button("jump", &["Space", "GamepadSouth"])
            .button("feed", &["Digit1"])
            .button("shoes", &["Digit2"])
    }
    fn restorable(saved: &World) -> Result<(), String> {
        match saved.try_resource::<SaveContract>().map(|c| c.version) {
            Some(SAVE_CONTRACT) => Ok(()),
            found => Err(format!(
                "its save contract is {}; this build reads version {SAVE_CONTRACT}",
                found.map_or("absent (version 1)".into(), |v| v.to_string())
            )),
        }
    }
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        // Coins first appear after setup; a fresh process restores them only if registered.
        w.register::<Collectable>();
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed);
        w.insert_resource(SaveContract {
            version: SAVE_CONTRACT,
        });
        // Colours are CSS, as in app.contract; the renderer decodes them to linear light.
        let sky = Color::hex("#bad6d1");
        w.insert_resource(Environment {
            background: Some(sky),
            fog: Some(Fog {
                color: Some(sky),
                ..Fog::new(0.02, 0.08)
            }),
            ..Environment::default()
        });
        w.spawn((
            Transform::default(),
            Mesh::plane(40.0, 40.0),
            Material::grid(Color::hex("#6f8486"), 1.0),
        ));
        let player = w.spawn_named(
            "player",
            (
                Transform::at(0.0, 0.9, 0.0),
                Mesh::capsule(0.4, 1.8),
                Material::hex("#e7aa59"),
                Character::new().ground(0.9).bounds_xz(-19.6..=19.6),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::default(),
                Camera::default(),
                Follow::new(player).offset(0.0, 9.0, 13.0).lag(0.15),
            ),
        );
        w.spawn_named(
            "sun",
            (
                Transform::at(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
                DirectionalLight::default(),
            ),
        );
        for (i, x) in [2.0, 6.0].into_iter().enumerate() {
            w.spawn_named(
                format!("plinth-{}", i + 1),
                (
                    Transform::at(x, 0.1, 0.0),
                    Mesh::cylinder(0.9, 0.2),
                    Material::hex("#95aaad"),
                ),
            );
            w.spawn_named(
                format!("beacon-{}", i + 1),
                (
                    Transform::at(x, 0.7, 0.0),
                    Mesh::sphere(0.5),
                    // Emission is linear light and may exceed 1: it blooms.
                    Material::hex("#81d7df").emissive(3., 0.45, 0.025),
                    Glow::default(),
                    Beacon::default(),
                ),
            );
        }
        let critter = w.spawn_named(
            "critter",
            (
                Transform::at(-4.0, 0.35, 0.0).looking_at(Vec3::new(0.0, 0.35, 0.0), Vec3::Y),
                Mesh::sphere(0.35),
                Material::hex("#b98ad6"),
                Critter::default(),
            ),
        );
        // A nose, so the way it faces shows.
        w.spawn((
            Parent(critter),
            Transform::at(0.0, 0.05, -0.32),
            Mesh::sphere(0.12),
            Material::hex("#6d4a85"),
        ));
        w.insert_resource(Shop::default());
        w.publish("total", w.count::<Beacon>(|_| true));
        w.publish("lit", 0);
        w.publish("near", "");
        publish_shop(w);
    }
    fn paused(args: &Options) -> bool {
        args.paused
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        w.character("player")
            .step(input.stick_xz("move"), input.pressed("jump"));
        let mut near = "";
        if let Some((_, mut beacon)) = w.nearest_xz_mut::<Beacon>("player", 1.5, |b| !b.lit) {
            if input.pressed("light") {
                beacon.lit = true;
                beacon.glow_to(1.0, 0.5);
            } else {
                near = beacon.name().unwrap_or_default();
            }
        }
        w.publish("lit", w.count::<Beacon>(|b| b.lit));
        w.publish("near", near);
        critter(w);
        coins(w);
        shop(w, input);
        publish_shop(w);
    }
}

/// Wake on the player's approach; then run from the nearest lit beacon, else
/// follow the player, else wander; owe coins while fed; starve when not.
fn critter(w: &mut World) {
    let (dt, tick) = (w.dt(), w.tick());
    let player = w.require::<Transform>("player").position;
    let lights: Vec<Vec3> = w
        .query::<(&Transform, &Beacon)>()
        .iter()
        .filter(|(_, (_, b))| b.lit)
        .map(|(_, (t, _))| t.position)
        .collect();
    let mut droppers = Vec::new();
    for (e, (pose, critter)) in w.query::<(&mut Transform, &mut Critter)>().iter() {
        let level = |p: Vec3| Vec3::new(p.x, pose.position.y, p.z);
        let from_player = level(player) - pose.position;
        if !critter.awake {
            if from_player.length() > 2.5 {
                continue;
            }
            critter.awake = true;
            critter.fed_at = tick;
            critter.drops = Spawner {
                next: tick + u64::from(COIN_EVERY_S * SmallGame::HZ),
                ..Spawner::every(u64::from(COIN_EVERY_S * SmallGame::HZ))
            };
        }
        if critter.starved || critter.hunger(tick) >= 1.0 {
            // It earns nothing more, but the coins it already owes still drop.
            critter.starved = true;
            if !critter.owed.is_empty() {
                droppers.push((e, pose.position));
            }
            continue;
        }
        // Its hunger and its next coin come on later ticks, which observation
        // cannot see: `clock settle` keeps running while it is awake and fed.
        w.busy("critter awake: hunger and coins pending");
        // Equal distances keep the first beacon in entity order.
        let nearest = lights.iter().map(|l| level(*l)).min_by(|a, b| {
            a.distance(pose.position)
                .total_cmp(&b.distance(pose.position))
        });
        let s = nearest
            .and_then(|light| steer::flee(pose, light, 3.0, 3.5, 5.0, dt))
            .unwrap_or_else(|| {
                if from_player.length() < 12.0 {
                    // Stop a little short of the player rather than inside it.
                    let short = level(player) - from_player.normalize_or_zero() * 1.2;
                    steer::arrive(pose, short, 2.0, 3.0, 5.0, dt)
                } else {
                    steer::wander(pose, &mut w.rng(), &mut critter.turn, 1.2, 2.0, dt)
                }
            });
        pose.position =
            (pose.position + s.velocity * dt).clamp(Vec3::splat(-19.6), Vec3::splat(19.6));
        pose.rotation = s.facing;
        while let Some(serial) = critter.drops.due(tick) {
            critter.owed.push(serial);
        }
        if !critter.owed.is_empty() {
            droppers.push((e, pose.position));
        }
    }
    // Five coins on the ground at most; owed ones wait in the critter, in order.
    for (critter, at) in droppers {
        while w.count::<Collectable>(|_| true) < 5 {
            let mut owed = w.require_mut::<Critter>(critter);
            if owed.owed.is_empty() {
                break;
            }
            let serial = owed.owed.remove(0);
            drop(owed);
            let at = Vec3::new(at.x + w.rand(-0.5..0.5), 0.06, at.z + w.rand(-0.5..0.5));
            w.spawn_named(
                format!("coin-{serial}"),
                (
                    Transform::at(at.x, at.y, at.z),
                    Mesh::cylinder(0.22, 0.06),
                    Material::hex("#f2c94c").emissive(0.6, 0.45, 0.1),
                    Collectable,
                ),
            );
        }
    }
}

/// Coins within 3 m fly to the player and are counted on arrival.
fn coins(w: &mut World) {
    let dt = w.dt();
    let player = w.require::<Transform>("player").position;
    let mut taken = Vec::new();
    for (coin, pose) in w.query::<&mut Transform>().with::<Collectable>().iter() {
        let d = drift(pose.position, player, 3.0, 6.0, dt);
        pose.position = d.position;
        if d.taken {
            taken.push(coin);
        }
    }
    for coin in taken {
        w.despawn(coin);
        let mut shop = w.resource_mut::<Shop>();
        shop.coins += 1;
        shop.earned += 1;
    }
}

/// Buy on the number keys (or a HUD button holding the same action).
fn shop(w: &mut World, input: &Input) {
    for (i, (_, _, action)) in ITEMS.iter().enumerate() {
        // `pressed`: a tap can go down and up between two ticks.
        if !input.pressed(action) {
            continue;
        }
        let Ok(price) = price(w, i) else { continue };
        if price > w.resource::<Shop>().coins {
            continue;
        }
        w.resource_mut::<Shop>().coins -= price;
        if i == 0 {
            let tick = w.tick();
            w.require_mut::<Critter>("critter").fed_at = tick;
        } else {
            let shoes = {
                let mut shop = w.resource_mut::<Shop>();
                shop.shoes += 1;
                shop.shoes
            };
            w.require_mut::<Character>("player").speed = 4.0 + shoes as f32;
        }
    }
}

fn publish_shop(w: &World) {
    let critter = match w.get::<Critter>("critter") {
        Some(c) if !c.awake => "asleep",
        Some(c) if c.starved => "starved",
        Some(c) if c.hunger(w.tick()) >= 0.5 => "hungry",
        Some(_) => "fed",
        None => "gone",
    };
    w.publish_record(&Hud {
        coins: w.resource::<Shop>().coins,
        critter: critter.into(),
        shop: shop_rows(w),
    });
}
