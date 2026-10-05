//! The player's garden: tiles, sheckles, seeds, the backpack, and the
//! commands the HUD sends.

use crate::crops::{crop, kind_of, CROPS};
use crate::garden::{self, now_ms, Census, Fruit, GardenClock, Item, Plant, TILE};
use crate::{feedback, shop};
use exact_game::character::Character;
use exact_game::*;

pub const START_SIZE: u16 = 6;
pub const MAX_SIZE: u16 = 256;
pub const START_SHECKLES: u64 = 20;
pub const WATER_CAPACITY: u8 = 3;
pub const FOOD_CAPACITY: u8 = 3;
pub const BARREL: Vec3 = Vec3::new(-2.0, 0.0, 0.0);

/// A short ladder of market requests: crop, quantity, bonus on top of value.
pub const ORDERS: &[(u8, u32, u64)] = &[
    (0, 1, 30),
    (1, 4, 400),
    (2, 5, 600),
    (3, 4, 1300),
    (4, 3, 2400),
];

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
    pub last: String,
    pub away: String,
    pub overview: bool,
    pub shop_dirty: bool,
    pub bag_dirty: bool,
}

impl Farm {
    pub fn new() -> Self {
        let mut seeds = vec![0; CROPS.len()];
        seeds[0] = 1;
        Farm {
            sheckles: START_SHECKLES,
            water: WATER_CAPACITY,
            seeds,
            held: Some(0),
            size: START_SIZE,
            tiles: vec![None; START_SIZE as usize * START_SIZE as usize],
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
    pub fn expand_cost(&self) -> u64 {
        (self.size as u64).pow(3) * 25
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
pub fn plant_held(w: &mut World, tile: [u16; 2]) -> Result<Entity, String> {
    let (kind, occupied) = {
        let farm = w.resource::<Farm>();
        (farm.held, farm.at(tile).is_some())
    };
    let kind = kind.ok_or("No seed in hand")?;
    if occupied {
        return Err("Something grows here".into());
    }
    if w.resource::<Farm>().seeds[kind as usize] == 0 {
        return Err(format!("No {} seeds", crop(kind).name));
    }
    let now = now_ms(w);
    let e = garden::plant(w, kind, tile, now);
    let mut farm = w.resource_mut::<Farm>();
    farm.seeds[kind as usize] -= 1;
    let i = farm.index(tile).unwrap();
    farm.tiles[i] = Some(e);
    if farm.seeds[kind as usize] == 0 {
        farm.held = (0..CROPS.len() as u8).find(|&k| farm.seeds[k as usize] > 0);
    }
    farm.last = format!("Planted {}", crop(kind).name);
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
    let mut farm = w.resource_mut::<Farm>();
    item.id = farm.next_item;
    farm.next_item += 1;
    farm.harvested += 1;
    farm.last = format!("Harvested {} ({}¢)", item.label(), item.value());
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
    if n > 0 {
        w.resource_mut::<Farm>().last = format!("Harvested {n} fruit");
    }
    if let Some((at, item)) = picked {
        feedback::harvest(w, at, &item);
    }
    n
}

pub fn sell(w: &World, id: Option<u32>) -> u64 {
    let mut farm = w.resource_mut::<Farm>();
    let sold: Vec<Item> = match id {
        Some(id) => match farm.bag.iter().position(|i| i.id == id) {
            Some(i) => vec![farm.bag.remove(i)],
            None => vec![],
        },
        None => std::mem::take(&mut farm.bag),
    };
    let total: u64 = sold.iter().map(Item::value).sum();
    farm.sheckles += total;
    farm.earned += total;
    if !sold.is_empty() {
        farm.last = format!("Sold {} for {}¢", sold.len(), total);
        farm.bag_dirty = true;
        farm.shop_dirty = true;
    }
    total
}

/// Compost exactly the chosen fruit; a stale ID or full pouch spends nothing.
pub fn compost(w: &World, id: u32) -> Result<String, String> {
    let mut farm = w.resource_mut::<Farm>();
    if farm.plant_food >= FOOD_CAPACITY {
        return Err("Plant-food pouch full · feed a growing plot first".into());
    }
    let i = farm
        .bag
        .iter()
        .position(|item| item.id == id)
        .ok_or("Fruit is no longer in the backpack")?;
    let item = farm.bag.remove(i);
    farm.plant_food += 1;
    farm.bag_dirty = true;
    Ok(format!(
        "Composted {} · +1 plant food instead of {}¢",
        item.label(),
        item.value()
    ))
}

/// Deliver one whole request, retaining unrelated fruit. Every fruit still
/// earns its full weight/mutation value; the request adds its one-time bonus.
pub fn deliver(w: &World) -> Result<String, String> {
    let mut farm = w.resource_mut::<Farm>();
    let &(kind, count, bonus) = ORDERS
        .get(farm.orders as usize)
        .ok_or("All market orders filled")?;
    if farm.bag.iter().filter(|i| i.kind == kind).count() < count as usize {
        return Err(format!("Bring {count} {} to the market", crop(kind).name));
    }
    let mut remaining = count;
    let mut paid = bonus;
    farm.bag.retain(|i| {
        if i.kind != kind || remaining == 0 {
            return true;
        }
        remaining -= 1;
        paid += i.value();
        false
    });
    farm.orders += 1;
    // The introductory requests must not wait on a rare seed's stock roll.
    // This is shop inventory at its ordinary price, not a free seed or reward.
    if let Some(&(next, _, _)) = ORDERS.get(farm.orders as usize) {
        let mut shop = w.resource_mut::<shop::Shop>();
        shop.stock[next as usize] = shop.stock[next as usize].max(1);
    }
    farm.sheckles += paid;
    farm.earned += paid;
    farm.shop_dirty = true;
    farm.bag_dirty = true;
    Ok(format!(
        "Delivered {count} {} · {paid}¢ including {bonus}¢ bonus",
        crop(kind).name
    ))
}

/// Grows the garden to `size` tiles per side, keeping every plant's tile.
pub fn resize(w: &mut World, size: u16) {
    let size = size.min(MAX_SIZE);
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
    let mut follow = w.require_mut::<Follow>("camera");
    *follow = if overview {
        let r = span.max(12.0);
        Follow::new(w.named("ground").unwrap())
            .offset(0.0, r * 0.9, r * 0.75)
            .lag(0.3)
    } else {
        Follow::new(w.named("player").unwrap())
            .offset(0.0, 9.0, 11.0)
            .lag(0.15)
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
            let kind = (planted as usize % CROPS.len()) as u8;
            let e = garden::plant(w, kind, [x, z], now);
            let mut farm = w.resource_mut::<Farm>();
            let i = farm.index([x, z]).unwrap();
            farm.tiles[i] = Some(e);
            planted += 1;
        }
    }
    w.resource_mut::<Farm>().last = format!("Filled {planted} tiles");
    planted
}

/// Lets `ms` of garden time pass at once, as if the player had been away.
pub fn away(w: &mut World, ms: u64) -> garden::Ran {
    let before = w.resource::<Census>().ripe;
    w.resource_mut::<GardenClock>().offline_ms += ms;
    let ran = garden::run_due(w, now_ms(w));
    let ripe = w.resource::<Census>().ripe;
    w.resource_mut::<Farm>().away = format!(
        "While you were away ({}): {} events, {} fruit ripened ({} ripe now)",
        crate::crops::clock(ms),
        ran.events,
        ripe.saturating_sub(before),
        ripe
    );
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
    let result: Result<String, String> = match verb {
        "" => return,
        "buy" => match kind_of(arg) {
            Some(k) => shop::buy(w, k).map(|_| format!("Bought {} seed", crop(k).name)),
            None => Err(format!("No seed named {arg}")),
        },
        "equip" => match kind_of(arg) {
            Some(k) if w.resource::<Farm>().seeds[k as usize] > 0 => {
                w.resource_mut::<Farm>().held = Some(k);
                Ok(format!("Holding {}", crop(k).name))
            }
            _ => Err(format!("No {arg} seeds")),
        },
        "sell" => match arg {
            "all" => Ok(format!("Sold for {}¢", sell(w, None))),
            id => id
                .parse()
                .map_err(|_| "Sell needs a fruit id or all".into())
                .map(|id| format!("Sold for {}¢", sell(w, Some(id)))),
        },
        "deliver" => deliver(w),
        "compost" => arg
            .parse::<u32>()
            .map_err(|_| "Choose a fruit to compost".into())
            .and_then(|id| compost(w, id)),
        "harvest" => Ok(format!("Harvested {}", harvest_all(w))),
        "page" => {
            let mut farm = w.resource_mut::<Farm>();
            let pages = farm.bag.len().div_ceil(crate::hud::BAG_PAGE).max(1) as u32;
            farm.bag_page = match arg {
                "next" => (farm.bag_page + 1).min(pages - 1),
                _ => farm.bag_page.saturating_sub(1),
            };
            farm.bag_dirty = true;
            Ok(format!("Backpack page {}", farm.bag_page + 1))
        }
        "expand" => {
            let cost = w.resource::<Farm>().expand_cost();
            let size = w.resource::<Farm>().size;
            if size >= MAX_SIZE {
                Err("The garden is as big as it gets".into())
            } else if w.resource::<Farm>().sheckles < cost {
                Err(format!("Expanding costs {cost}¢"))
            } else {
                w.resource_mut::<Farm>().sheckles -= cost;
                resize(w, size + 2);
                Ok(format!("The garden is {0}×{0}", size + 2))
            }
        }
        "zoom" => {
            let o = !w.resource::<Farm>().overview;
            w.resource_mut::<Farm>().overview = o;
            lay_ground(w);
            Ok((if o { "Overview" } else { "Close" }).into())
        }
        // The art pass's camera; the other looks keep theirs.
        "closeup" => Ok(crate::pass::toggle_closeup(w)),
        "fill" => Ok(format!("Planted {}", fill(w, arg.parse().unwrap_or(100)))),
        "away" => {
            let ran = away(w, arg.parse::<u64>().unwrap_or(0) * 1000);
            Ok(format!("{} events while away", ran.events))
        }
        _ => Err(format!("Unknown command {cmd}")),
    };
    match result {
        Ok(line) => {
            w.log(format!("{cmd}: {line}"));
            let mut farm = w.resource_mut::<Farm>();
            farm.last = line;
            farm.shop_dirty = true;
        }
        Err(why) => {
            w.log(format!("{cmd}: refused: {why}"));
            w.resource_mut::<Farm>().last = why;
        }
    }
}

fn bearing(delta: Vec3) -> String {
    let direction = if delta.x.abs() >= delta.z.abs() {
        if delta.x > 0.0 {
            "east (D)"
        } else {
            "west (A)"
        }
    } else if delta.z > 0.0 {
        "south (S)"
    } else {
        "north (W)"
    };
    let metres = delta.length().round().max(1.0) as u32;
    format!("{direction} · about {metres} m")
}

/// A held seed needs somewhere to go without hiding this tile's harvest prompt.
/// Called only when publishing the HUD, never for every simulation tick. The
/// bounded tile table is already authoritative; no saved navigation cache.
pub fn planting_guidance(w: &World) -> String {
    let Some(player) = w.global_position("player") else {
        return String::new();
    };
    let Some(tile) = tile_at(w, player) else {
        return String::new();
    };
    let farm = w.resource::<Farm>();
    if farm.held.is_none() || farm.at(tile).is_none() {
        return String::new();
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
    match nearest {
        Some(([x, z], delta)) => format!("Empty plot {}, {}: {}", x + 1, z + 1, bearing(delta)),
        None if farm.size < MAX_SIZE => "Garden full · expand to add empty plots".into(),
        None => "Garden full · no empty plots".into(),
    }
}

/// The player's tile and what E would do there, or a direction back to it.
pub fn prompt(w: &World) -> (Option<[u16; 2]>, String) {
    let Some(player) = w.global_position("player") else {
        return (None, String::new());
    };
    let Some(tile) = tile_at(w, player) else {
        // Aim at the nearest plot's centre, beyond its boundary. The larger
        // axis always leads inward; once it is crossed the prompt can turn
        // the corner. No scan of the garden or saved navigation state.
        let last = w.resource::<Farm>().size.saturating_sub(1) as f32;
        let x = (player.x / TILE).round().clamp(0.0, last) * TILE;
        let z = -(-player.z / TILE).round().clamp(0.0, last) * TILE;
        let delta = Vec3::new(x - player.x, 0.0, z - player.z);
        return (None, format!("Return to garden: {}", bearing(delta)));
    };
    let now = now_ms(w);
    let farm = w.resource::<Farm>();
    let text = match farm.at(tile) {
        None => match farm.held {
            Some(k) => format!(
                "E: plant {} ({} left)",
                crop(k).name,
                farm.seeds[k as usize]
            ),
            None => "Buy seeds in the shop".into(),
        },
        Some(p) => {
            let (kind, stage, planted, span) = w
                .get::<Plant>(p)
                .map(|p| (p.kind, p.stage, p.planted, p.grow_ms))
                .unwrap_or_default();
            let c = crop(kind);
            if stage < 4 {
                let left = (planted + span).saturating_sub(now);
                format!("{} growing · {}", c.name, crate::crops::clock(left))
            } else {
                let fruits = garden::fruits_of(w, p);
                let ripe = fruits.iter().filter(|f| f.1).count();
                if ripe > 0 {
                    format!("E: harvest {ripe} {}", c.name)
                } else {
                    let next = fruits.iter().map(|f| f.2).min().unwrap_or(now);
                    format!(
                        "{} fruiting · {}",
                        c.name,
                        crate::crops::clock(next.saturating_sub(now))
                    )
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

pub fn refill_guidance(w: &World) -> String {
    if at_barrel(w) {
        "At the blue barrel · R refills the can".into()
    } else {
        let p = w.global_position("player").unwrap_or_default();
        let d = BARREL - p;
        format!("Blue barrel: {}", bearing(Vec3::new(d.x, 0.0, d.z)))
    }
}

pub fn water_here(w: &World) -> Result<String, String> {
    if w.resource::<Farm>().water == 0 {
        return Err("Can empty · refill at the blue barrel with R".into());
    }
    let plant = w
        .global_position("player")
        .and_then(|p| tile_at(w, p))
        .and_then(|tile| w.resource::<Farm>().at(tile))
        .ok_or("Stand on a growing plot to water")?;
    if !garden::needs_water(w, plant) {
        return Err("Already watered or ripe · wait for the next growth".into());
    }
    garden::water(w, plant, now_ms(w));
    w.resource_mut::<Farm>().water -= 1;
    feedback::cue(
        w,
        feedback::Cue::Water,
        garden::plant_center(w.require::<Plant>(plant).tile),
    );
    Ok(format!(
        "Watered {} · remaining wait cut by 25%",
        crop(w.require::<Plant>(plant).kind).name
    ))
}

pub fn refill(w: &World) -> Result<String, String> {
    if !at_barrel(w) {
        return Err(refill_guidance(w));
    }
    let mut farm = w.resource_mut::<Farm>();
    if farm.water == WATER_CAPACITY {
        return Err("The watering can is full".into());
    }
    farm.water = WATER_CAPACITY;
    drop(farm);
    feedback::cue(w, feedback::Cue::Refill, BARREL);
    Ok("Watering can refilled · 3 doses".into())
}

pub fn feed_here(w: &World) -> Result<String, String> {
    if w.resource::<Farm>().plant_food == 0 {
        return Err("Compost a backpack fruit for plant food".into());
    }
    let plant = w
        .global_position("player")
        .and_then(|p| tile_at(w, p))
        .and_then(|tile| w.resource::<Farm>().at(tile))
        .ok_or("Stand on a growing plot to feed")?;
    if !garden::needs_feed(w, plant) {
        return Err("Already fed or ripe · wait for new growth".into());
    }
    garden::feed(w, plant);
    w.resource_mut::<Farm>().plant_food -= 1;
    feedback::cue(
        w,
        feedback::Cue::Feed,
        garden::plant_center(w.require::<Plant>(plant).tile),
    );
    Ok(format!(
        "Fed {} · next fruit weighs 25% more",
        crop(w.require::<Plant>(plant).kind).name
    ))
}

/// E on a tile: harvest what is ripe there, or plant the held seed.
pub fn act(w: &mut World, tile: [u16; 2]) {
    let here = w.resource::<Farm>().at(tile);
    match here {
        Some(p) => {
            if harvest_plant(w, p) == 0 {
                w.resource_mut::<Farm>().last = "Nothing ripe yet".into();
            }
        }
        None => {
            if let Err(why) = plant_held(w, tile) {
                w.resource_mut::<Farm>().last = why;
            }
        }
    }
}
