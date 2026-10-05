//! The player's garden: tiles, sheckles, seeds, the backpack, and the
//! commands the HUD sends.

use crate::crops::{balance, Balance};
use crate::garden::{self, now_ms, Census, Fruit, GardenClock, Item, Plant, TILE};
use crate::hud::{secs, Away, Barrel, Bearing, Described, Note, Planting, Prompt};
use crate::{feedback, shop};
use exact_game::character::Character;
use exact_game::*;

pub const BARREL: Vec3 = Vec3::new(-2.0, 0.0, 0.0);

#[derive(Default, Resource)]
pub struct Farm {
    pub sheckles: u64,
    pub water: u8,
    pub plant_food: u8,
    /// Seeds in hand, by crop kind.
    pub seeds: Vec<u32>,
    pub held: Option<u8>,
    pub bag: Vec<Item>,
    /// The backpack page the HUD shows, newest first.
    pub bag_page: u32,
    pub next_item: u32,
    /// Tiles per side.
    pub size: u16,
    /// The plant on each tile, row-major over `size`.
    pub tiles: Vec<Option<Entity>>,
    pub harvested: u64,
    pub earned: u64,
    /// The next market request; also the saved receipt for each paid bonus.
    pub orders: u32,
    /// What the last action did, for the HUD to word.
    pub last: Note,
    /// What the garden did while the player was away.
    pub away: Away,
    pub overview: bool,
    pub shop_dirty: bool,
    pub bag_dirty: bool,
}

impl Farm {
    pub fn new(b: &Balance) -> Self {
        let mut seeds = vec![0; b.crops.len()];
        seeds[0] = 1;
        let size = b.farm.start_size;
        Farm {
            sheckles: b.farm.start_sheckles,
            water: b.farm.water,
            seeds,
            held: Some(0),
            size,
            tiles: vec![None; size as usize * size as usize],
            shop_dirty: true,
            bag_dirty: true,
            ..Farm::default()
        }
    }
    fn index(&self, tile: [u16; 2]) -> Option<usize> {
        (tile[0] < self.size && tile[1] < self.size)
            .then(|| tile[1] as usize * self.size as usize + tile[0] as usize)
    }
    pub fn at(&self, tile: [u16; 2]) -> Option<Entity> {
        self.index(tile).and_then(|i| self.tiles[i])
    }
    pub fn expand_cost(&self, b: &Balance) -> u64 {
        (self.size as u64).pow(3) * b.farm.expand_cost
    }
}

pub fn clear_tile(w: &World, tile: [u16; 2]) {
    let mut farm = w.resource_mut::<Farm>();
    if let Some(i) = farm.index(tile) {
        farm.tiles[i] = None;
    }
}

/// The tile under a world position, if inside the garden.
pub fn tile_at(w: &World, p: Vec3) -> Option<[u16; 2]> {
    let size = w.resource::<Farm>().size as f32;
    let (x, z) = ((p.x / TILE).round(), (-p.z / TILE).round());
    (x >= 0.0 && z >= 0.0 && x < size && z < size).then_some([x as u16, z as u16])
}

const PLOT_EDGES: [(&str, f32, f32); 4] = [
    ("plot-north", 0.0, -0.96),
    ("plot-south", 0.0, 0.96),
    ("plot-west", -0.96, 0.0),
    ("plot-east", 0.96, 0.0),
];

pub fn create_plot_outline(w: &mut World) {
    for (name, x, _) in PLOT_EDGES {
        let (width, depth) = if x == 0.0 { (1.96, 0.04) } else { (0.04, 1.96) };
        w.spawn_named(
            name,
            (
                Transform::default(),
                Mesh::plane(width, depth),
                Material::default(),
                Visible(false),
            ),
        );
    }
    show_plot(w, Some([0, 0]));
}

/// Only the active plot and its bounded fruit slots are read. Unchanged
/// components stay untouched, so a timer publication cannot dirty the scene.
pub fn show_plot(w: &World, tile: Option<[u16; 2]>) {
    let plant = tile.and_then(|tile| w.resource::<Farm>().at(tile));
    let color = match plant {
        None => [0.2, 0.85, 1.0],
        Some(p) if garden::fruits_of(w, p).iter().any(|fruit| fruit.1) => [0.35, 1.0, 0.3],
        Some(p) if garden::is_fed(w, p) => [0.8, 0.55, 1.0],
        Some(p) if !garden::needs_water(w, p) => [0.2, 0.65, 1.0],
        Some(_) => [1.0, 0.7, 0.15],
    };
    let material = garden::paint(color).emissive(color[0] * 0.15, color[1] * 0.15, color[2] * 0.15);
    for (name, x, z) in PLOT_EDGES {
        if w.require::<Visible>(name).0 != tile.is_some() {
            w.require_mut::<Visible>(name).0 = tile.is_some();
        }
        let Some(tile) = tile else { continue };
        let position = garden::tile_center(tile) + Vec3::new(x, 0.025, z);
        if w.require::<Transform>(name).position != position {
            w.require_mut::<Transform>(name).position = position;
        }
        if *w.require::<Material>(name) != material {
            *w.require_mut::<Material>(name) = material;
        }
    }
}

/// Plants the held seed on a tile. Refuses an occupied tile or an empty hand.
pub fn plant_held(w: &mut World, tile: [u16; 2]) -> Result<Entity, Note> {
    let (kind, occupied) = {
        let farm = w.resource::<Farm>();
        (farm.held, farm.at(tile).is_some())
    };
    let kind = kind.ok_or_else(|| Note::new("no_seed_in_hand"))?;
    if occupied {
        return Err(Note::new("occupied"));
    }
    let b = balance(w);
    if w.resource::<Farm>().seeds[kind as usize] == 0 {
        return Err(Note::new("no_seeds").crop(&b.crop(kind).name));
    }
    let now = now_ms(w);
    let e = garden::plant(w, kind, tile, now);
    let mut farm = w.resource_mut::<Farm>();
    farm.seeds[kind as usize] -= 1;
    let i = farm.index(tile).unwrap();
    farm.tiles[i] = Some(e);
    if farm.seeds[kind as usize] == 0 {
        farm.held = (0..b.crops.len() as u8).find(|&k| farm.seeds[k as usize] > 0);
    }
    farm.last = Note::new("planted").crop(&b.crop(kind).name);
    farm.shop_dirty = true;
    drop(farm);
    feedback::cue(w, feedback::Cue::Plant, garden::plant_center(tile));
    Ok(e)
}

/// Harvests every ripe fruit of a plant into the backpack.
pub fn harvest_plant(w: &mut World, plant: Entity) -> u32 {
    let now = now_ms(w);
    let ripe: Vec<Entity> = garden::fruits_of(w, plant)
        .into_iter()
        .filter(|f| f.1)
        .map(|f| f.0)
        .collect();
    let mut n = 0;
    let mut picked = None;
    for f in ripe {
        let at = w.require::<Transform>(f).position;
        if let Some(item) = garden::pick(w, f, now) {
            picked = Some((at, item.clone()));
            stow(w, item);
            n += 1;
        }
    }
    if let Some((at, item)) = picked {
        feedback::harvest(w, at, &item);
    }
    n
}

fn stow(w: &World, mut item: Item) {
    let b = balance(w);
    let mut farm = w.resource_mut::<Farm>();
    item.id = farm.next_item;
    farm.next_item += 1;
    farm.harvested += 1;
    farm.last = Note::new("harvested")
        .fruit(Described::of(&b, &item))
        .coins(item.value(&b));
    farm.bag.push(item);
    farm.bag_dirty = true;
}

/// Harvests every ripe fruit in the garden, in entity order.
pub fn harvest_all(w: &mut World) -> u32 {
    let now = now_ms(w);
    let ripe: Vec<Entity> = w
        .query::<&Fruit>()
        .iter()
        .filter(|(_, f)| f.ripe)
        .map(|(e, _)| e)
        .collect();
    let mut n = 0;
    let mut picked = None;
    for f in ripe {
        let at = w.require::<Transform>(f).position;
        if let Some(item) = garden::pick(w, f, now) {
            picked = Some((at, item.clone()));
            stow(w, item);
            n += 1;
        }
    }
    if let Some((at, item)) = picked {
        feedback::harvest(w, at, &item);
    }
    n
}

pub fn sell(w: &World, id: Option<u32>) -> u64 {
    let b = balance(w);
    let mut farm = w.resource_mut::<Farm>();
    let sold: Vec<Item> = match id {
        Some(id) => match farm.bag.iter().position(|i| i.id == id) {
            Some(i) => vec![farm.bag.remove(i)],
            None => vec![],
        },
        None => std::mem::take(&mut farm.bag),
    };
    let total: u64 = sold.iter().map(|i| i.value(&b)).sum();
    farm.sheckles += total;
    farm.earned += total;
    if !sold.is_empty() {
        farm.bag_dirty = true;
        farm.shop_dirty = true;
    }
    total
}

/// Compost exactly the chosen fruit; a stale ID or full pouch spends nothing.
pub fn compost(w: &World, id: u32) -> Result<Note, Note> {
    let b = balance(w);
    let mut farm = w.resource_mut::<Farm>();
    if farm.plant_food >= b.farm.food {
        return Err(Note::new("pouch_full"));
    }
    let i = farm
        .bag
        .iter()
        .position(|item| item.id == id)
        .ok_or_else(|| Note::new("fruit_gone"))?;
    let item = farm.bag.remove(i);
    farm.plant_food += 1;
    farm.bag_dirty = true;
    Ok(Note::new("composted")
        .fruit(Described::of(&b, &item))
        .coins(item.value(&b)))
}

/// Deliver one whole request, retaining unrelated fruit. Every fruit still
/// earns its full weight/mutation value; the request adds its one-time bonus.
pub fn deliver(w: &World) -> Result<Note, Note> {
    let b = balance(w);
    let mut farm = w.resource_mut::<Farm>();
    let (kind, count, bonus) = b
        .order(farm.orders)
        .ok_or_else(|| Note::new("orders_done"))?;
    let name = &b.crop(kind).name;
    if farm.bag.iter().filter(|i| i.kind == kind).count() < count as usize {
        return Err(Note::new("bring").crop(name).count(count as u64));
    }
    let mut remaining = count;
    let mut paid = bonus;
    farm.bag.retain(|i| {
        if i.kind != kind || remaining == 0 {
            return true;
        }
        remaining -= 1;
        paid += i.value(&b);
        false
    });
    farm.orders += 1;
    // The introductory requests must not wait on a rare seed's stock roll.
    // This is shop inventory at its ordinary price, not a free seed or reward.
    if let Some((next, _, _)) = b.order(farm.orders) {
        let mut shop = w.resource_mut::<shop::Shop>();
        shop.stock[next as usize] = shop.stock[next as usize].max(1);
    }
    farm.sheckles += paid;
    farm.earned += paid;
    farm.shop_dirty = true;
    farm.bag_dirty = true;
    Ok(Note::new("delivered")
        .crop(name)
        .count(count as u64)
        .coins(paid)
        .bonus(bonus))
}

/// Grows the garden to `size` tiles per side, keeping every plant's tile.
pub fn resize(w: &mut World, size: u16) {
    let size = size.min(balance(w).farm.max_size);
    {
        let mut farm = w.resource_mut::<Farm>();
        if size <= farm.size {
            return;
        }
        let old = farm.size as usize;
        let mut tiles = vec![None; size as usize * size as usize];
        for z in 0..old {
            for x in 0..old {
                tiles[z * size as usize + x] = farm.tiles[z * old + x];
            }
        }
        farm.tiles = tiles;
        farm.size = size;
    }
    lay_ground(w);
    crate::art::regrow(w);
}

/// Sizes the ground, the player's bounds and the overview camera to the garden.
pub fn lay_ground(w: &World) {
    let (size, overview) = {
        let farm = w.resource::<Farm>();
        (farm.size as f32, farm.overview)
    };
    let span = size * TILE;
    let mid = (size - 1.0) * TILE / 2.0;
    // Two metres around the plots keep the barrel wholly on the garden's pad.
    *w.require_mut::<Mesh>("ground") = Mesh::plane(span + 4.0, span + 4.0);
    w.require_mut::<Transform>("ground").position = Vec3::new(mid, 0.0, -mid);
    crate::art::resize(w, span, mid);
    w.require_mut::<Character>("player").bounds = Some([-span, span]);
    let rig = balance(w).camera.clone();
    let mut follow = w.require_mut::<Follow>("camera");
    *follow = if overview {
        let r = span.max(rig.overview_min);
        Follow::new(w.named("ground").unwrap())
            .offset(0.0, r * rig.overview[0], r * rig.overview[1])
            .lag(rig.overview_lag)
    } else {
        let [x, y, z] = rig.offset;
        Follow::new(w.named("player").unwrap())
            .offset(x, y, z)
            .lag(rig.lag)
    };
}

/// Plants `n` seeds for free, growing the garden to fit: the scale test.
/// Kinds cycle through the catalogue so every growth path is exercised.
pub fn fill(w: &mut World, n: u32) -> u32 {
    let plants = w.resource::<Census>().plants;
    let need = ((plants + n) as f64).sqrt().ceil() as u16;
    resize(w, need);
    let now = now_ms(w);
    let size = w.resource::<Farm>().size;
    let mut planted = 0;
    'tiles: for z in 0..size {
        for x in 0..size {
            if planted == n {
                break 'tiles;
            }
            if w.resource::<Farm>().at([x, z]).is_some() {
                continue;
            }
            let kind = (planted as usize % balance(w).crops.len()) as u8;
            let e = garden::plant(w, kind, [x, z], now);
            let mut farm = w.resource_mut::<Farm>();
            let i = farm.index([x, z]).unwrap();
            farm.tiles[i] = Some(e);
            planted += 1;
        }
    }
    planted
}

/// Lets `ms` of garden time pass at once, as if the player had been away.
pub fn away(w: &mut World, ms: u64) -> garden::Ran {
    let before = w.resource::<Census>().ripe;
    w.resource_mut::<GardenClock>().offline_ms += ms;
    let ran = garden::run_due(w, now_ms(w));
    let ripe = w.resource::<Census>().ripe;
    w.resource_mut::<Farm>().away = Away {
        s: secs(ms),
        events: ran.events,
        ripened: ripe.saturating_sub(before),
        ripe,
    };
    w.log(format!("away {ms} ms: {} events", ran.events));
    ran
}

/// Applies the host's wall clock: a later session's epoch, ahead of where
/// this world's own clock says it is, means the garden was away that long.
pub fn observe_epoch(w: &mut World, epoch: f64) {
    if epoch <= 0.0 {
        return;
    }
    let (seen, seen_tick) = {
        let c = w.resource::<GardenClock>();
        (c.epoch, c.epoch_tick)
    };
    if seen == epoch {
        return;
    }
    if seen > 0.0 {
        let believed = seen + (w.tick() - seen_tick.min(w.tick())) as f64 * 1000.0 / w.hz() as f64;
        let gap = epoch - believed;
        if gap >= 1000.0 {
            away(w, gap as u64);
        }
    }
    let tick = w.tick();
    let mut c = w.resource_mut::<GardenClock>();
    c.epoch = epoch;
    c.epoch_tick = tick;
}

/// Applies one HUD command: `buy carrot`, `equip carrot`, `sell all`,
/// `sell 12`, `harvest all`, `page next`, `expand`, `zoom`, `fill 2000`,
/// `away 3600`.
pub fn command(w: &mut World, cmd: &str) {
    let mut words = cmd.split_whitespace();
    let verb = words.next().unwrap_or("");
    let arg = words.next().unwrap_or("");
    let b = balance(w);
    let result: Result<Note, Note> = match verb {
        "" => return,
        "buy" => match b.kind_of(arg) {
            Some(k) => shop::buy(w, k).map(|_| Note::new("bought").crop(&b.crop(k).name)),
            None => Err(Note::new("no_seed_named").word(arg)),
        },
        "equip" => match b.kind_of(arg) {
            Some(k) if w.resource::<Farm>().seeds[k as usize] > 0 => {
                w.resource_mut::<Farm>().held = Some(k);
                Ok(Note::new("holding").crop(&b.crop(k).name))
            }
            _ => Err(Note::new("no_seeds").word(arg)),
        },
        "sell" => match arg {
            "all" => Ok(Note::new("sold").coins(sell(w, None))),
            id => id
                .parse()
                .map_err(|_| Note::new("sell_needs_id"))
                .map(|id| Note::new("sold").coins(sell(w, Some(id)))),
        },
        "deliver" => deliver(w),
        "compost" => arg
            .parse::<u32>()
            .map_err(|_| Note::new("choose_compost"))
            .and_then(|id| compost(w, id)),
        "harvest" => Ok(Note::new("harvested_all").count(harvest_all(w) as u64)),
        "page" => {
            let mut farm = w.resource_mut::<Farm>();
            let pages = farm.bag.len().div_ceil(crate::hud::BAG_PAGE).max(1) as u32;
            farm.bag_page = match arg {
                "next" => (farm.bag_page + 1).min(pages - 1),
                _ => farm.bag_page.saturating_sub(1),
            };
            farm.bag_dirty = true;
            Ok(Note::new("page").count(farm.bag_page as u64 + 1))
        }
        "expand" => {
            let cost = w.resource::<Farm>().expand_cost(&b);
            let size = w.resource::<Farm>().size;
            if size >= b.farm.max_size {
                Err(Note::new("max_size"))
            } else if w.resource::<Farm>().sheckles < cost {
                Err(Note::new("expand_costs").coins(cost))
            } else {
                w.resource_mut::<Farm>().sheckles -= cost;
                resize(w, size + 2);
                Ok(Note::new("expanded").count(size as u64 + 2))
            }
        }
        "zoom" => {
            let o = !w.resource::<Farm>().overview;
            w.resource_mut::<Farm>().overview = o;
            lay_ground(w);
            Ok(Note::new(if o { "overview" } else { "close" }))
        }
        // The art pass's camera; the other looks keep theirs.
        "closeup" => Ok(crate::pass::toggle_closeup(w)),
        "fill" => Ok(Note::new("filled").count(fill(w, arg.parse().unwrap_or(100)) as u64)),
        "away" => {
            let ran = away(w, arg.parse::<u64>().unwrap_or(0) * 1000);
            Ok(Note::new("away").count(ran.events))
        }
        _ => Err(Note::new("unknown").word(cmd)),
    };
    match result {
        Ok(note) => {
            w.log(format!("{cmd}: {}", note.what));
            let mut farm = w.resource_mut::<Farm>();
            farm.last = note;
            farm.shop_dirty = true;
        }
        Err(why) => {
            w.log(format!("{cmd}: refused: {}", why.what));
            w.resource_mut::<Farm>().last = why;
        }
    }
}

fn bearing(delta: Vec3) -> Bearing {
    let dir = if delta.x.abs() >= delta.z.abs() {
        if delta.x > 0.0 {
            "east"
        } else {
            "west"
        }
    } else if delta.z > 0.0 {
        "south"
    } else {
        "north"
    };
    Bearing {
        dir: dir.into(),
        metres: delta.length().round().max(1.0) as u32,
    }
}

/// A held seed needs somewhere to go without hiding this tile's harvest prompt.
/// Called only when publishing the HUD, never for every simulation tick. The
/// bounded tile table is already authoritative; no saved navigation cache.
pub fn planting_guidance(w: &World) -> Planting {
    let Some(player) = w.global_position("player") else {
        return Planting::default();
    };
    let Some(tile) = tile_at(w, player) else {
        return Planting::default();
    };
    let max = balance(w).farm.max_size;
    let farm = w.resource::<Farm>();
    if farm.held.is_none() || farm.at(tile).is_none() {
        return Planting::default();
    }
    let mut nearest = None;
    let mut distance = f32::INFINITY;
    for (i, plant) in farm.tiles.iter().enumerate() {
        if plant.is_some() {
            continue;
        }
        let tile = [
            (i % farm.size as usize) as u16,
            (i / farm.size as usize) as u16,
        ];
        let delta = garden::tile_center(tile) - player;
        let d = delta.x * delta.x + delta.z * delta.z;
        // Equal distances keep the first row-major tile, also after restore.
        if d < distance {
            distance = d;
            nearest = Some((tile, Vec3::new(delta.x, 0.0, delta.z)));
        }
    }
    let what = |what: &str| Planting {
        what: what.into(),
        ..Planting::default()
    };
    match nearest {
        Some(([x, z], delta)) => Planting {
            x: x as u32,
            z: z as u32,
            way: bearing(delta),
            ..what("empty")
        },
        None if farm.size < max => what("full"),
        None => what("maxed"),
    }
}

/// The player's tile and what E would do there, or a direction back to it.
pub fn prompt(w: &World) -> (Option<[u16; 2]>, Prompt) {
    let Some(player) = w.global_position("player") else {
        return (None, Prompt::default());
    };
    let Some(tile) = tile_at(w, player) else {
        // Aim at the nearest plot's centre, beyond its boundary. The larger
        // axis always leads inward; once it is crossed the prompt can turn
        // the corner. No scan of the garden or saved navigation state.
        let last = w.resource::<Farm>().size.saturating_sub(1) as f32;
        let x = (player.x / TILE).round().clamp(0.0, last) * TILE;
        let z = -(-player.z / TILE).round().clamp(0.0, last) * TILE;
        let delta = Vec3::new(x - player.x, 0.0, z - player.z);
        let way = bearing(delta);
        return (
            None,
            Prompt {
                what: "return".into(),
                way,
                ..Prompt::default()
            },
        );
    };
    let now = now_ms(w);
    let b = balance(w);
    let farm = w.resource::<Farm>();
    let prompt = |what: &str, crop: &str| Prompt {
        what: what.into(),
        crop: crop.into(),
        ..Prompt::default()
    };
    let text = match farm.at(tile) {
        None => match farm.held {
            Some(k) => Prompt {
                count: farm.seeds[k as usize],
                ..prompt("plant", &b.crop(k).name)
            },
            None => prompt("buy", ""),
        },
        Some(p) => {
            let (kind, stage, planted, span) = w
                .get::<Plant>(p)
                .map(|p| (p.kind, p.stage, p.planted, p.grow_ms))
                .unwrap_or_default();
            let c = b.crop(kind);
            if stage < 4 {
                let left = (planted + span).saturating_sub(now);
                Prompt {
                    s: secs(left),
                    ..prompt("growing", &c.name)
                }
            } else {
                let fruits = garden::fruits_of(w, p);
                let ripe = fruits.iter().filter(|f| f.1).count();
                if ripe > 0 {
                    Prompt {
                        count: ripe as u32,
                        ..prompt("harvest", &c.name)
                    }
                } else {
                    let next = fruits.iter().map(|f| f.2).min().unwrap_or(now);
                    Prompt {
                        s: secs(next.saturating_sub(now)),
                        ..prompt("fruiting", &c.name)
                    }
                }
            }
        }
    };
    (Some(tile), text)
}

pub fn at_barrel(w: &World) -> bool {
    w.global_position("player").is_some_and(|p| {
        let delta = p - BARREL;
        delta.x * delta.x + delta.z * delta.z <= 2.5 * 2.5
    })
}

/// The blue barrel: whether the player stands at it, and the way there.
pub fn barrel(w: &World) -> Barrel {
    let p = w.global_position("player").unwrap_or_default();
    let d = BARREL - p;
    Barrel {
        at: at_barrel(w),
        way: bearing(Vec3::new(d.x, 0.0, d.z)),
    }
}

pub fn water_here(w: &World) -> Result<Note, Note> {
    if w.resource::<Farm>().water == 0 {
        return Err(Note::new("can_empty"));
    }
    let plant = w
        .global_position("player")
        .and_then(|p| tile_at(w, p))
        .and_then(|tile| w.resource::<Farm>().at(tile))
        .ok_or_else(|| Note::new("stand_to_water"))?;
    if !garden::needs_water(w, plant) {
        return Err(Note::new("already_watered"));
    }
    garden::water(w, plant, now_ms(w));
    w.resource_mut::<Farm>().water -= 1;
    feedback::cue(
        w,
        feedback::Cue::Water,
        garden::plant_center(w.require::<Plant>(plant).tile),
    );
    Ok(Note::new("watered").crop(&balance(w).crop(w.require::<Plant>(plant).kind).name))
}

pub fn refill(w: &World) -> Result<Note, Note> {
    if !at_barrel(w) {
        return Err(Note::new("barrel").way(barrel(w).way));
    }
    let full = balance(w).farm.water;
    let mut farm = w.resource_mut::<Farm>();
    if farm.water == full {
        return Err(Note::new("can_full"));
    }
    farm.water = full;
    drop(farm);
    feedback::cue(w, feedback::Cue::Refill, BARREL);
    Ok(Note::new("refilled").count(full as u64))
}

pub fn feed_here(w: &World) -> Result<Note, Note> {
    if w.resource::<Farm>().plant_food == 0 {
        return Err(Note::new("no_food"));
    }
    let plant = w
        .global_position("player")
        .and_then(|p| tile_at(w, p))
        .and_then(|tile| w.resource::<Farm>().at(tile))
        .ok_or_else(|| Note::new("stand_to_feed"))?;
    if !garden::needs_feed(w, plant) {
        return Err(Note::new("already_fed"));
    }
    garden::feed(w, plant);
    w.resource_mut::<Farm>().plant_food -= 1;
    feedback::cue(
        w,
        feedback::Cue::Feed,
        garden::plant_center(w.require::<Plant>(plant).tile),
    );
    Ok(Note::new("fed").crop(&balance(w).crop(w.require::<Plant>(plant).kind).name))
}

/// E on a tile: harvest what is ripe there, or plant the held seed.
pub fn act(w: &mut World, tile: [u16; 2]) {
    let here = w.resource::<Farm>().at(tile);
    match here {
        Some(p) => {
            if harvest_plant(w, p) == 0 {
                w.resource_mut::<Farm>().last = Note::new("nothing_ripe");
            }
        }
        None => {
            if let Err(why) = plant_held(w, tile) {
                w.resource_mut::<Farm>().last = why;
            }
        }
    }
}
