//! Garden time, the schedule of everything that will happen in it, plants and
//! fruit. Growth is event-driven: a tick touches only what is due, so an hour
//! of growth (or an afternoon away) costs the events in it, not ticks × plants.

use crate::crops::{self, crop};
use exact_game::*;

/// Metres between tile centres.
pub const TILE: f32 = 2.0;

/// What becomes due at a garden time.
#[derive(Clone, Copy, Default, PartialEq, Debug, Data)]
pub enum Due {
    #[default]
    Nothing,
    /// A plant reaches its next stage.
    Grow(Entity),
    /// A fruit ripens and rolls its mutations.
    Ripen(Entity),
    /// The seed shop restocks.
    Restock,
    /// The weather changes.
    Weather,
}

#[derive(Clone, Copy, Default, Debug, Data)]
pub struct Entry {
    pub at: u64,
    pub seq: u64,
    pub due: Due,
}
impl Entry {
    fn before(&self, other: &Entry) -> bool {
        (self.at, self.seq) < (other.at, other.seq)
    }
}

/// A binary min-heap of due events, ordered by (garden ms, insertion order),
/// kept in a plain `Vec` so it saves as data. Same inputs, same order: a live
/// session and an offline catch-up over the same span process the same events
/// in the same order and so draw the same random numbers.
#[derive(Default, Resource)]
pub struct Schedule {
    pub heap: Vec<Entry>,
    pub seq: u64,
    /// Events processed since the game began.
    pub processed: u64,
}

impl Schedule {
    pub fn push(&mut self, at: u64, due: Due) {
        self.seq += 1;
        self.heap.push(Entry {
            at,
            seq: self.seq,
            due,
        });
        let mut i = self.heap.len() - 1;
        while i > 0 {
            let parent = (i - 1) / 2;
            if !self.heap[i].before(&self.heap[parent]) {
                break;
            }
            self.heap.swap(i, parent);
            i = parent;
        }
    }
    pub fn peek(&self) -> Option<u64> {
        self.heap.first().map(|e| e.at)
    }
    pub fn pop(&mut self) -> Option<Entry> {
        if self.heap.is_empty() {
            return None;
        }
        let last = self.heap.len() - 1;
        self.heap.swap(0, last);
        let top = self.heap.pop();
        let n = self.heap.len();
        let mut i = 0;
        loop {
            let (l, r) = (2 * i + 1, 2 * i + 2);
            let mut m = i;
            if l < n && self.heap[l].before(&self.heap[m]) {
                m = l;
            }
            if r < n && self.heap[r].before(&self.heap[m]) {
                m = r;
            }
            if m == i {
                break;
            }
            self.heap.swap(i, m);
            i = m;
        }
        top
    }
}

/// Garden time: world time plus every span the garden grew while away.
#[derive(Default, Resource)]
pub struct GardenClock {
    pub offline_ms: u64,
    /// The host's Unix time (ms) at its clock zero, as last bound, and the
    /// world tick at which it was seen: together, an estimate of "now" that a
    /// later session's epoch is compared with.
    pub epoch: f64,
    pub epoch_tick: u64,
}

pub fn now_ms(w: &World) -> u64 {
    w.tick() * 1000 / w.hz() as u64 + w.resource::<GardenClock>().offline_ms
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Data)]
pub enum Sky {
    #[default]
    Clear,
    Rain,
    Snow,
    Storm,
}
impl Sky {
    pub fn name(self) -> &'static str {
        match self {
            Sky::Clear => "Clear",
            Sky::Rain => "Rain",
            Sky::Snow => "Snow",
            Sky::Storm => "Thunderstorm",
        }
    }
}

#[derive(Default, Resource)]
pub struct Weather {
    pub sky: Sky,
    pub until: u64,
    pub events: u32,
}

/// A planted seed. `stage` counts 0..=4; 4 is mature and bears fruit.
#[derive(Default, Component)]
pub struct Plant {
    pub kind: u8,
    pub tile: [u16; 2],
    pub planted: u64,
    /// Effective growth span; watering preserves progress and shortens what remains.
    pub grow_ms: u64,
    pub watered: bool,
    /// Passed to the first fruit batch, then cleared for ordinary regrowth.
    pub fed: bool,
    pub stage: u8,
    /// The fruit in each slot. `World::children` scans every entity, so the
    /// plant keeps its own list.
    pub fruits: Vec<Option<Entity>>,
}

/// A fruit on a plant. Unripe until `ripe_at`.
#[derive(Default, Component)]
pub struct Fruit {
    pub kind: u8,
    pub plant: Option<Entity>,
    pub slot: u8,
    pub set_at: u64,
    pub ripe_at: u64,
    pub ripe: bool,
    pub watered: bool,
    pub fed: bool,
    pub weight: f32,
    pub muts: u8,
}

/// Counters the HUD reads without scanning the world.
#[derive(Default, Resource)]
pub struct Census {
    pub plants: u32,
    pub fruit: u32,
    pub ripe: u32,
    pub mutated: u32,
}

/// Tiles run +x and −z from the origin, so the garden lies in front of
/// the following camera.
pub fn tile_center(tile: [u16; 2]) -> Vec3 {
    Vec3::new(tile[0] as f32 * TILE, 0.0, -(tile[1] as f32) * TILE)
}

/// Leave the centre of the interaction tile clear for the player. Use the
/// same anchor for the stem and its world-space fruit, including regrowth.
pub(crate) fn plant_center(tile: [u16; 2]) -> Vec3 {
    tile_center(tile) + Vec3::new(0.65, 0.0, -0.35)
}

/// A material from an sRGB-ish authored colour (materials are linear).
pub fn paint(c: [f32; 3]) -> Material {
    Material::rgb(c[0] * c[0], c[1] * c[1], c[2] * c[2])
}

fn plant_scale(stage: u8) -> f32 {
    0.25 + 0.75 * stage as f32 / 4.0
}

fn plant_pose(kind: u8, tile: [u16; 2], scale: f32) -> Transform {
    let h = crop(kind).height;
    let mut t = Transform::at(0.0, h * scale / 2.0, 0.0).with_scale(scale);
    t.position += plant_center(tile);
    t
}

/// Spawns a plant on a tile at garden time `at` and schedules its first stage.
pub fn plant(w: &mut World, kind: u8, tile: [u16; 2], at: u64) -> Entity {
    let c = crop(kind);
    let p = Plant {
        kind,
        tile,
        planted: at,
        grow_ms: c.grow_s as u64 * 1000,
        watered: false,
        fed: false,
        stage: 0,
        fruits: vec![None; c.slots as usize],
    };
    let e = w.spawn((
        plant_pose(kind, tile, plant_scale(0)),
        Mesh::asset(format!("plant-{kind}.model")),
        Material::default(),
        p,
    ));
    let step = c.grow_s as u64 * 1000 / 4;
    w.resource_mut::<Schedule>().push(at + step, Due::Grow(e));
    w.resource_mut::<Census>().plants += 1;
    e
}

fn fruit_offset(kind: u8, slot: u8) -> Vec3 {
    let c = crop(kind);
    if c.slots == 1 {
        return Vec3::new(0.0, c.height / 2.0 + c.fruit_size * 0.6, 0.0);
    }
    let a = slot as f32 / c.slots as f32 * std::f32::consts::TAU;
    let y = c.height * (0.05 + 0.4 * ((slot % 3) as f32 / 2.0));
    Vec3::new(math::cos(a) * 0.42, y, math::sin(a) * 0.42)
}

/// Spawns an unripe fruit in a plant's slot and schedules its ripening. A
/// single-harvest crop's fruit ripens as it appears.
///
/// Fruit is placed in world space, not parented to its plant: a mature plant
/// never moves. After the hierarchy and render fixes a still parented garden
/// draws as cheaply, but an hour's seek at 21,100 plants still costs more
/// parented: 3.0 s against 0.57 s on web, 0.43 s against 0.20 s on Linux (diary).
pub fn bear(w: &mut World, plant: Entity, kind: u8, slot: u8, at: u64) -> Entity {
    let c = crop(kind);
    let ripe_at = at + c.fruit_s as u64 * 1000;
    let base = w.require::<Plant>(plant).tile;
    let fed = w.require::<Plant>(plant).fed;
    let fruit = Fruit {
        kind,
        plant: Some(plant),
        slot,
        set_at: at,
        ripe_at,
        ripe: false,
        watered: false,
        fed,
        weight: c.weight,
        muts: 0,
    };
    let mut pose = Transform::at(0.0, c.height / 2.0, 0.0).with_scale(0.4);
    pose.position += plant_center(base) + fruit_offset(kind, slot);
    let e = w.spawn((
        pose,
        Mesh::asset(format!("fruit-{kind}.model")),
        paint([0.55, 0.75, 0.35]),
        fruit,
    ));
    w.require_mut::<Plant>(plant).fruits[slot as usize] = Some(e);
    w.resource_mut::<Census>().fruit += 1;
    if ripe_at <= at {
        ripen(w, e);
    } else {
        w.resource_mut::<Schedule>().push(ripe_at, Due::Ripen(e));
    }
    e
}

/// Rolls a ripening fruit's size and mutations against the weather now.
fn ripen(w: &mut World, e: Entity) {
    let sky = w.resource::<Weather>().sky;
    let (kind, muts, weight) = {
        let mut rng = w.rng();
        let kind = w.require::<Fruit>(e).kind;
        let mut muts = 0;
        let roll = rng.next_f32();
        if roll < 0.001 {
            muts |= crops::RAINBOW;
        } else if roll < 0.011 {
            muts |= crops::GOLD;
        }
        match sky {
            Sky::Clear => {}
            Sky::Rain => {
                if rng.chance(0.5) {
                    muts |= crops::WET;
                }
            }
            Sky::Snow => {
                if rng.chance(0.4) {
                    muts |= crops::CHILLED;
                } else if rng.chance(0.1) {
                    muts |= crops::FROZEN;
                }
            }
            Sky::Storm => {
                if rng.chance(0.5) {
                    muts |= crops::WET;
                }
                if rng.chance(0.03) {
                    muts |= crops::SHOCKED;
                }
            }
        }
        let mut weight = crop(kind).weight * rng.range(0.8..1.4);
        if rng.chance(0.02) {
            weight *= 3.0;
        }
        if w.require::<Fruit>(e).fed {
            weight *= 1.25;
        }
        (kind, muts, weight)
    };
    {
        let mut fruit = w.require_mut::<Fruit>(e);
        fruit.ripe = true;
        fruit.muts = muts;
        fruit.weight = weight;
    }
    let size = (weight / crop(kind).weight).sqrt();
    w.require_mut::<Transform>(e).scale = Vec3::splat(size);
    // The classic look's colour; the art pass draws its own (pass::present).
    let color = crops::fruit_color(kind, muts);
    *w.require_mut::<Material>(e) = if muts & (crops::GOLD | crops::RAINBOW | crops::SHOCKED) != 0 {
        paint(color).emissive(color[0] * 0.5, color[1] * 0.5, color[2] * 0.5)
    } else {
        paint(color)
    };
    let mut census = w.resource_mut::<Census>();
    census.ripe += 1;
    if muts != 0 {
        census.mutated += 1;
    }
}

/// What the garden did while an event loop ran.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Ran {
    pub events: u64,
    pub ripened: u32,
    pub restocks: u32,
    pub weathers: u32,
}

/// Processes every event due at or before `until`, in order. Used by every
/// tick and by an offline catch-up alike.
pub fn run_due(w: &mut World, until: u64) -> Ran {
    let mut ran = Ran::default();
    loop {
        let entry = {
            let mut s = w.resource_mut::<Schedule>();
            match s.peek() {
                Some(at) if at <= until => s.pop(),
                _ => None,
            }
        };
        let Some(entry) = entry else { break };
        ran.events += 1;
        match entry.due {
            Due::Nothing => {}
            Due::Grow(e) => grow(w, e, entry.at),
            Due::Ripen(e) => {
                if w.get::<Fruit>(e)
                    .is_some_and(|f| !f.ripe && f.ripe_at == entry.at)
                {
                    ripen(w, e);
                    ran.ripened += 1;
                }
            }
            Due::Restock => {
                crate::shop::restock(w, entry.at);
                ran.restocks += 1;
            }
            Due::Weather => {
                change_weather(w, entry.at);
                ran.weathers += 1;
            }
        }
    }
    w.resource_mut::<Schedule>().processed += ran.events;
    ran
}

fn grow(w: &mut World, e: Entity, at: u64) {
    // Watering leaves the old event in the heap. Only the current deadline may
    // advance a stage; an obsolete event cannot bear a second set of fruit.
    let Some((kind, tile, stage, planted, span)) = w
        .get::<Plant>(e)
        .filter(|p| p.stage < 4 && at == p.planted + p.grow_ms * (p.stage as u64 + 1) / 4)
        .map(|p| (p.kind, p.tile, p.stage + 1, p.planted, p.grow_ms))
    else {
        return;
    };
    w.require_mut::<Plant>(e).stage = stage;
    // Classic scales one model; the art pass draws one per stage (pass::present).
    *w.require_mut::<Transform>(e) = plant_pose(kind, tile, plant_scale(stage));
    let c = crop(kind);
    if stage < 4 {
        w.resource_mut::<Schedule>()
            .push(planted + span * (stage as u64 + 1) / 4, Due::Grow(e));
    } else {
        for slot in 0..c.slots {
            bear(w, e, kind, slot, at);
        }
        w.require_mut::<Plant>(e).fed = false;
    }
}

/// Feeding visits this plot's bounded fruit slots, never the whole garden.
/// It changes the weight at ripening without drawing another random number.
pub fn needs_feed(w: &World, plant: Entity) -> bool {
    w.get::<Plant>(plant).is_some_and(|p| {
        if p.stage < 4 {
            !p.fed
        } else {
            p.fruits
                .iter()
                .flatten()
                .any(|&e| w.get::<Fruit>(e).is_some_and(|f| !f.ripe && !f.fed))
        }
    })
}

pub fn is_fed(w: &World, plant: Entity) -> bool {
    w.get::<Plant>(plant).is_some_and(|p| {
        p.fed
            || p.fruits
                .iter()
                .flatten()
                .any(|&e| w.get::<Fruit>(e).is_some_and(|f| f.fed))
    })
}

pub fn feed(w: &World, plant: Entity) {
    let mut p = w.require_mut::<Plant>(plant);
    if p.stage < 4 {
        p.fed = true;
    } else {
        for &e in p.fruits.iter().flatten() {
            if let Some(mut f) = w.get_mut::<Fruit>(e).filter(|f| !f.ripe) {
                f.fed = true;
            }
        }
    }
}

/// One dose per growing plant, or per new unripe fruit. The effective start
/// moves forward by a quarter of elapsed time, preserving visual progress;
/// the deadline moves nearer by a quarter of the remaining time. No world scan.
pub fn needs_water(w: &World, plant: Entity) -> bool {
    w.get::<Plant>(plant).is_some_and(|p| {
        if p.stage < 4 {
            !p.watered
        } else {
            p.fruits
                .iter()
                .flatten()
                .any(|&e| w.get::<Fruit>(e).is_some_and(|f| !f.ripe && !f.watered))
        }
    })
}

pub fn water(w: &World, plant: Entity, now: u64) {
    let mut p = w.require_mut::<Plant>(plant);
    if p.stage < 4 && !p.watered {
        p.watered = true;
        p.planted += now.saturating_sub(p.planted) / 4;
        p.grow_ms = p.grow_ms * 3 / 4;
        w.resource_mut::<Schedule>().push(
            p.planted + p.grow_ms * (p.stage as u64 + 1) / 4,
            Due::Grow(plant),
        );
    } else if p.stage == 4 {
        for &e in p.fruits.iter().flatten() {
            let Some(mut f) = w.get_mut::<Fruit>(e).filter(|f| !f.ripe && !f.watered) else {
                continue;
            };
            let span = (f.ripe_at - f.set_at) * 3 / 4;
            f.set_at += now.saturating_sub(f.set_at) / 4;
            f.ripe_at = f.set_at + span;
            f.watered = true;
            w.resource_mut::<Schedule>().push(f.ripe_at, Due::Ripen(e));
        }
    }
}

pub fn change_weather(w: &mut World, at: u64) {
    let (sky, len) = if w.resource::<Weather>().sky == Sky::Clear {
        let roll = w.rand(0.0f32..1.0);
        let sky = if roll < 0.6 {
            Sky::Rain
        } else if roll < 0.85 {
            Sky::Snow
        } else {
            Sky::Storm
        };
        (sky, w.rand(60u64..121))
    } else {
        (Sky::Clear, w.rand(180u64..421))
    };
    let until = at + len * 1000;
    {
        let mut weather = w.resource_mut::<Weather>();
        weather.sky = sky;
        weather.until = until;
        weather.events += 1;
    }
    if sky != Sky::Clear {
        w.log(format!("weather: {} for {len} s", sky.name()));
    }
    w.resource_mut::<Schedule>().push(until, Due::Weather);
}

/// One harvested fruit, in the backpack.
#[derive(Clone, Default, Debug, Data)]
pub struct Item {
    pub id: u32,
    pub kind: u8,
    pub weight: f32,
    pub muts: u8,
    pub fed: bool,
}
impl Item {
    pub fn value(&self) -> u64 {
        crops::fruit_value(self.kind, self.weight, self.muts)
    }
    pub fn label(&self) -> String {
        let m = crops::mutation_names(self.muts);
        let label = if m.is_empty() {
            crop(self.kind).name.to_string()
        } else {
            format!("{m} {}", crop(self.kind).name)
        };
        if self.fed {
            format!("Fed {label}")
        } else {
            label
        }
    }
}

/// Picks a ripe fruit: despawns it, regrows its slot or removes a
/// single-harvest plant. Returns the harvested item without an id.
pub fn pick(w: &mut World, fruit: Entity, at: u64) -> Option<Item> {
    let f = w
        .get::<Fruit>(fruit)
        .filter(|f| f.ripe)
        .map(|f| (f.kind, f.weight, f.muts, f.plant, f.slot, f.fed))?;
    let (kind, weight, muts, plant, slot, fed) = f;
    w.despawn(fruit);
    if let Some(mut p) = plant.and_then(|p| w.get_mut::<Plant>(p)) {
        p.fruits[slot as usize] = None;
    }
    {
        let mut census = w.resource_mut::<Census>();
        census.fruit -= 1;
        census.ripe -= 1;
        if muts != 0 {
            census.mutated -= 1;
        }
    }
    if let Some(plant) = plant {
        if crop(kind).regrows {
            bear(w, plant, kind, slot, at);
        } else {
            remove_plant(w, plant);
        }
    }
    Some(Item {
        id: 0,
        kind,
        weight,
        muts,
        fed,
    })
}

pub fn remove_plant(w: &mut World, plant: Entity) {
    for (fruit, ripe, _) in fruits_of(w, plant) {
        w.despawn(fruit);
        let mut census = w.resource_mut::<Census>();
        census.fruit -= 1;
        census.ripe -= ripe as u32;
    }
    let tile = w.get::<Plant>(plant).map(|p| p.tile);
    if let Some(tile) = tile {
        crate::farm::clear_tile(w, tile);
        w.despawn(plant);
        w.resource_mut::<Census>().plants -= 1;
    }
}

/// Every fruit on a plant: (entity, ripe, ripe_at).
pub fn fruits_of(w: &World, plant: Entity) -> Vec<(Entity, bool, u64)> {
    let Some(slots) = w.get::<Plant>(plant).map(|p| p.fruits.clone()) else {
        return vec![];
    };
    slots
        .into_iter()
        .flatten()
        .filter_map(|e| w.get::<Fruit>(e).map(|f| (e, f.ripe, f.ripe_at)))
        .collect()
}

/// Smooth growth, the way a naive clone animates it: every tick, every
/// growing plant and unripe fruit gets a new scale. O(entities) per tick.
pub fn animate(w: &World, now: u64) {
    // The art pass grows its grounded stage models from the same clock (pass::present).
    for (_, (t, p)) in w.query::<(&mut Transform, &Plant)>().iter() {
        if p.stage < 4 {
            let c = crop(p.kind);
            let f = ((now - p.planted.min(now)) as f32 / p.grow_ms.max(1) as f32).min(1.0);
            let s = 0.25 + 0.75 * f;
            t.scale = Vec3::splat(s);
            t.position.y = c.height * s / 2.0;
        }
    }
    for (_, (t, f)) in w.query::<(&mut Transform, &Fruit)>().iter() {
        if !f.ripe {
            let span = (f.ripe_at - f.set_at).max(1) as f32;
            let k = ((now - f.set_at.min(now)) as f32 / span).min(1.0);
            t.scale = Vec3::splat(0.4 + 0.5 * k);
        }
    }
}

/// When the first weather arrives: three to seven clear minutes.
pub fn change_after(w: &World) -> u64 {
    let at = w.rand(180u64..421) * 1000;
    w.resource_mut::<Weather>().until = at;
    at
}
